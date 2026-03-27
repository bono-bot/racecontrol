use tokio::sync::RwLock;

use super::types::GalleryEntry;

/// Compute cosine similarity between two L2-normalized 512-D embeddings.
///
/// Since the vectors are already L2-normalized (from ArcFace output),
/// cosine similarity equals the dot product.
pub fn cosine_similarity(a: &[f32; 512], b: &[f32; 512]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// In-memory embedding gallery for face recognition.
///
/// Holds a set of known face embeddings and matches query embeddings
/// via cosine similarity with a configurable threshold.
pub struct Gallery {
    entries: RwLock<Vec<GalleryEntry>>,
    threshold: f32,
}

impl Gallery {
    /// Create a new gallery with initial entries and similarity threshold.
    pub fn new(entries: Vec<GalleryEntry>, threshold: f32) -> Self {
        Self {
            entries: RwLock::new(entries),
            threshold,
        }
    }

    /// Find the best matching person for a query embedding.
    ///
    /// Returns `Some((person_id, person_name, similarity))` if the best match
    /// exceeds the threshold, or `None` if no match is found.
    pub async fn find_match(&self, query: &[f32; 512]) -> Option<(i64, String, f32)> {
        let entries = self.entries.read().await;
        let mut best: Option<(i64, String, f32)> = None;

        for entry in entries.iter() {
            let sim = cosine_similarity(query, &entry.embedding);
            if sim > self.threshold {
                if best.as_ref().map_or(true, |(_, _, best_sim)| sim > *best_sim) {
                    best = Some((entry.person_id, entry.person_name.clone(), sim));
                }
            }
        }

        best
    }

    /// Replace all gallery entries (used for periodic reload from DB).
    #[allow(dead_code)]
    pub async fn reload(&self, new_entries: Vec<GalleryEntry>) {
        let mut entries = self.entries.write().await;
        *entries = new_entries;
    }

    /// Return the number of entries in the gallery.
    #[allow(dead_code)]
    pub async fn entry_count(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Add a single entry to the gallery (used after enrollment, avoids full reload).
    pub async fn add_entry(&self, entry: GalleryEntry) {
        let mut entries = self.entries.write().await;
        entries.push(entry);
    }

    /// Remove all entries for a person (used after person deletion).
    pub async fn remove_person(&self, person_id: i64) {
        let mut entries = self.entries.write().await;
        entries.retain(|e| e.person_id != person_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_embedding(val: f32) -> [f32; 512] {
        // Create an L2-normalized embedding with a dominant first component
        let mut emb = [0.0_f32; 512];
        emb[0] = val;
        // L2 normalize
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for v in emb.iter_mut() {
                *v /= norm;
            }
        }
        emb
    }

    fn make_orthogonal_pair() -> ([f32; 512], [f32; 512]) {
        let mut a = [0.0_f32; 512];
        let mut b = [0.0_f32; 512];
        a[0] = 1.0;
        b[1] = 1.0;
        (a, b)
    }

    #[test]
    fn test_cosine_identical_returns_one() {
        let emb = make_embedding(1.0);
        let sim = cosine_similarity(&emb, &emb);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "identical vectors should have cosine sim ~1.0, got {sim}"
        );
    }

    #[test]
    fn test_cosine_orthogonal_returns_zero() {
        let (a, b) = make_orthogonal_pair();
        let sim = cosine_similarity(&a, &b);
        assert!(
            sim.abs() < 1e-5,
            "orthogonal vectors should have cosine sim ~0.0, got {sim}"
        );
    }

    #[tokio::test]
    async fn test_find_match_empty_gallery_returns_none() {
        let gallery = Gallery::new(vec![], 0.45);
        let query = make_embedding(1.0);
        assert!(gallery.find_match(&query).await.is_none());
    }

    #[tokio::test]
    async fn test_find_match_above_threshold_returns_some() {
        let emb = make_embedding(1.0);
        let entries = vec![GalleryEntry {
            person_id: 42,
            person_name: "Alice".to_string(),
            embedding: emb,
        }];
        let gallery = Gallery::new(entries, 0.45);
        let result = gallery.find_match(&emb).await;
        assert!(result.is_some(), "should match identical embedding");
        let (id, name, sim) = result.unwrap();
        assert_eq!(id, 42);
        assert_eq!(name, "Alice");
        assert!(sim > 0.45, "similarity {sim} should exceed threshold");
    }

    #[tokio::test]
    async fn test_find_match_below_threshold_returns_none() {
        let (a, b) = make_orthogonal_pair();
        let entries = vec![GalleryEntry {
            person_id: 1,
            person_name: "Bob".to_string(),
            embedding: a,
        }];
        let gallery = Gallery::new(entries, 0.45);
        let result = gallery.find_match(&b).await;
        assert!(result.is_none(), "orthogonal embedding should not match");
    }

    #[tokio::test]
    async fn test_add_entry() {
        let gallery = Gallery::new(vec![], 0.45);
        assert_eq!(gallery.entry_count().await, 0);

        gallery
            .add_entry(GalleryEntry {
                person_id: 1,
                person_name: "Alice".to_string(),
                embedding: make_embedding(1.0),
            })
            .await;

        assert_eq!(gallery.entry_count().await, 1);
    }

    #[tokio::test]
    async fn test_remove_person() {
        let entries = vec![
            GalleryEntry {
                person_id: 1,
                person_name: "Alice".to_string(),
                embedding: make_embedding(1.0),
            },
            GalleryEntry {
                person_id: 1,
                person_name: "Alice".to_string(),
                embedding: make_embedding(0.9),
            },
            GalleryEntry {
                person_id: 2,
                person_name: "Bob".to_string(),
                embedding: make_embedding(0.5),
            },
        ];
        let gallery = Gallery::new(entries, 0.45);
        assert_eq!(gallery.entry_count().await, 3);

        gallery.remove_person(1).await;
        assert_eq!(gallery.entry_count().await, 1, "only Bob's entry should remain");
    }

    #[tokio::test]
    async fn test_remove_person_nonexistent() {
        let entries = vec![GalleryEntry {
            person_id: 1,
            person_name: "Alice".to_string(),
            embedding: make_embedding(1.0),
        }];
        let gallery = Gallery::new(entries, 0.45);
        gallery.remove_person(999).await;
        assert_eq!(gallery.entry_count().await, 1, "should not remove anything");
    }
}
