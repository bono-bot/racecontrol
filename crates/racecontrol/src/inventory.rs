//! v29.0 Phase 23: Predictive spare parts inventory based on RUL data.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct SparePartRecommendation {
    pub component: String,
    pub part_name: String,
    pub estimated_cost_paise: i64,
    pub urgency: String,
    pub reason: String,
    pub pod_ids_affected: Vec<u8>,
}

/// Cost lookup table (INR paise) — Hermes 405B MMA insight
pub fn component_cost_paise(component: &str) -> i64 {
    match component {
        "GPU" => 1_200_000,         // GPU fan: 12,000 INR
        "Storage" => 550_000,       // SSD: 5,500 INR
        "Memory" => 480_000,        // RAM stick: 4,800 INR
        "CPU" => 300_000,           // CPU cooler: 3,000 INR
        "Network" => 200_000,       // Network cable/adapter: 2,000 INR
        "PowerSupply" => 800_000,   // PSU: 8,000 INR
        "Peripherals" => 1_500_000, // Wheelbase repair: 15,000 INR
        "Cooling" => 250_000,       // Fan replacement: 2,500 INR
        "Software" => 120_000,      // Software fix labor: 1,200 INR
        _ => 500_000,               // Default: 5,000 INR
    }
}

/// Generate spare part recommendations based on component RUL data.
///
/// `rul_data` is a slice of (pod_id, component_name, rul_hours).
pub fn recommend_parts(rul_data: &[(u8, String, f32)]) -> Vec<SparePartRecommendation> {
    let mut recommendations = Vec::new();

    // Group by component, find pods with low RUL
    let mut by_component: HashMap<String, Vec<(u8, f32)>> = HashMap::new();
    for (pod_id, component, rul_hours) in rul_data {
        by_component
            .entry(component.clone())
            .or_default()
            .push((*pod_id, *rul_hours));
    }

    for (component, pods) in &by_component {
        let critical_pods: Vec<u8> = pods
            .iter()
            .filter(|(_, h)| *h < 168.0)
            .map(|(id, _)| *id)
            .collect(); // <1 week
        let warning_pods: Vec<u8> = pods
            .iter()
            .filter(|(_, h)| *h >= 168.0 && *h < 720.0)
            .map(|(id, _)| *id)
            .collect(); // 1 week - 1 month

        if !critical_pods.is_empty() {
            recommendations.push(SparePartRecommendation {
                component: component.clone(),
                part_name: format!("{} replacement part", component),
                estimated_cost_paise: component_cost_paise(component)
                    * critical_pods.len() as i64,
                urgency: "Critical".into(),
                reason: format!(
                    "{} pod(s) predicted to fail within 1 week",
                    critical_pods.len()
                ),
                pod_ids_affected: critical_pods,
            });
        }
        if !warning_pods.is_empty() {
            recommendations.push(SparePartRecommendation {
                component: component.clone(),
                part_name: format!("{} replacement part", component),
                estimated_cost_paise: component_cost_paise(component)
                    * warning_pods.len() as i64,
                urgency: "Medium".into(),
                reason: format!(
                    "{} pod(s) predicted to need replacement within 1 month",
                    warning_pods.len()
                ),
                pod_ids_affected: warning_pods,
            });
        }
    }

    recommendations
}
