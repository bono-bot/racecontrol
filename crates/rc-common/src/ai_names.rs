/// Pool of realistic AI driver names, shuffled per session.
/// Covers international diversity: Italian, British, Japanese, Indian, French, German, Brazilian, etc.
pub const AI_DRIVER_NAMES: &[&str] = &[
    "Marco Rossi", "James Mitchell", "Carlos Mendes", "Yuki Tanaka",
    "Liam O'Brien", "Alessandro Bianchi", "Felix Weber", "Raj Patel",
    "Pierre Dubois", "Hans Mueller", "Takeshi Kimura", "David Chen",
    "Matteo Ferrari", "Oliver Thompson", "Fernando Almeida", "Kenji Sato",
    "Arjun Sharma", "Jean-Paul Laurent", "Stefan Braun", "Lucas Silva",
    "Ethan Williams", "Vincenzo Moretti", "Hiroshi Nakamura", "Ravi Kumar",
    "Antoine Mercier", "Maximilian Richter", "Tomoko Hayashi", "Andre Costa",
    "Gabriel Martinez", "Noah Anderson", "Sergio Conti", "Akira Yamamoto",
    "Vikram Singh", "Christoph Hartmann", "Raphael Bertrand", "Thiago Oliveira",
    "Sebastian Kraft", "Ivan Petrov", "Diego Herrera", "Samuel Johnson",
    "Roberto Marchetti", "Kazuki Watanabe", "Anil Gupta", "Julien Moreau",
    "Henrik Lindberg", "Mateus Santos", "William Clarke", "Lorenzo Romano",
    "Taro Fujimoto", "Prashant Reddy", "Nicolas Lefevre", "Kurt Zimmerman",
    "Renato Barbosa", "Michael O'Connor", "Emilio Gentile", "Sho Taniguchi",
    "Deepak Verma", "Philippe Girard", "Markus Bauer", "Leonardo Ricci",
];

/// Pick N unique AI driver names from the pool, shuffled randomly.
pub fn pick_ai_names(count: usize) -> Vec<String> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    let mut names: Vec<&str> = AI_DRIVER_NAMES.to_vec();
    names.shuffle(&mut rng);
    names.into_iter().take(count).map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_driver_names_pool_size() {
        assert!(
            AI_DRIVER_NAMES.len() >= 50,
            "AI_DRIVER_NAMES must have at least 50 names, got {}",
            AI_DRIVER_NAMES.len()
        );
    }

    #[test]
    fn test_pick_ai_names_exact_count() {
        let names = pick_ai_names(5);
        assert_eq!(names.len(), 5, "pick_ai_names(5) must return exactly 5 names");
        // All unique
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), 5, "All 5 names must be unique");
    }

    #[test]
    fn test_pick_ai_names_zero() {
        let names = pick_ai_names(0);
        assert!(names.is_empty(), "pick_ai_names(0) must return empty vec");
    }
}
