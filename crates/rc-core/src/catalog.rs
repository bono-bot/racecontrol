use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
pub struct TrackEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub country: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CarEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
}

// ─── Featured Tracks ─────────────────────────────────────────────────────────

const FEATURED_TRACKS: &[TrackEntry] = &[
    // F1 Circuits
    TrackEntry { id: "spa", name: "Spa-Francorchamps", category: "F1 Circuits", country: "Belgium" },
    TrackEntry { id: "monza", name: "Monza", category: "F1 Circuits", country: "Italy" },
    TrackEntry { id: "ks_silverstone", name: "Silverstone", category: "F1 Circuits", country: "UK" },
    TrackEntry { id: "ks_red_bull_ring", name: "Red Bull Ring", category: "F1 Circuits", country: "Austria" },
    TrackEntry { id: "spielberg", name: "Spielberg", category: "F1 Circuits", country: "Austria" },
    TrackEntry { id: "ks_barcelona", name: "Barcelona", category: "F1 Circuits", country: "Spain" },
    TrackEntry { id: "monaco", name: "Monaco", category: "F1 Circuits", country: "Monaco" },
    TrackEntry { id: "interlagos", name: "Interlagos", category: "F1 Circuits", country: "Brazil" },
    TrackEntry { id: "bahrain", name: "Bahrain", category: "F1 Circuits", country: "Bahrain" },
    TrackEntry { id: "yas_marina_circuit-day", name: "Yas Marina (Day)", category: "F1 Circuits", country: "UAE" },
    TrackEntry { id: "albert-park_acu", name: "Albert Park", category: "F1 Circuits", country: "Australia" },
    TrackEntry { id: "china-gp", name: "Shanghai", category: "F1 Circuits", country: "China" },
    TrackEntry { id: "cota", name: "Circuit of the Americas", category: "F1 Circuits", country: "USA" },
    TrackEntry { id: "jeddah21", name: "Jeddah", category: "F1 Circuits", country: "Saudi Arabia" },
    TrackEntry { id: "lasvegas23", name: "Las Vegas", category: "F1 Circuits", country: "USA" },
    TrackEntry { id: "singapore", name: "Singapore", category: "F1 Circuits", country: "Singapore" },
    TrackEntry { id: "fn_losail", name: "Losail", category: "F1 Circuits", country: "Qatar" },
    TrackEntry { id: "baku_2022", name: "Baku", category: "F1 Circuits", country: "Azerbaijan" },
    TrackEntry { id: "vrc_mexico", name: "Mexico City", category: "F1 Circuits", country: "Mexico" },
    TrackEntry { id: "rt_suzuka", name: "Suzuka", category: "F1 Circuits", country: "Japan" },
    TrackEntry { id: "imola", name: "Imola", category: "F1 Circuits", country: "Italy" },
    TrackEntry { id: "ks_zandvoort", name: "Zandvoort", category: "F1 Circuits", country: "Netherlands" },
    // Real Circuits
    TrackEntry { id: "ks_nordschleife", name: "Nordschleife", category: "Real Circuits", country: "Germany" },
    TrackEntry { id: "ks_nurburgring", name: "Nurburgring GP", category: "Real Circuits", country: "Germany" },
    TrackEntry { id: "ks_laguna_seca", name: "Laguna Seca", category: "Real Circuits", country: "USA" },
    TrackEntry { id: "mugello", name: "Mugello", category: "Real Circuits", country: "Italy" },
    TrackEntry { id: "ks_brands_hatch", name: "Brands Hatch", category: "Real Circuits", country: "UK" },
    TrackEntry { id: "phillip_island_circuit", name: "Phillip Island", category: "Real Circuits", country: "Australia" },
    TrackEntry { id: "daytona_2017", name: "Daytona", category: "Real Circuits", country: "USA" },
    TrackEntry { id: "sx_lemans", name: "Le Mans", category: "Real Circuits", country: "France" },
    // Indian Circuits
    TrackEntry { id: "kari_motor_speedway", name: "Kari Motor Speedway", category: "Indian Circuits", country: "India" },
    TrackEntry { id: "madras_international_circuit", name: "Madras Motor Race Track", category: "Indian Circuits", country: "India" },
    TrackEntry { id: "india", name: "Buddh International Circuit", category: "Indian Circuits", country: "India" },
    // Street / Touge
    TrackEntry { id: "shuto_revival_project_beta", name: "Shuto Expressway (SRP)", category: "Street / Touge", country: "Japan" },
    TrackEntry { id: "haruna", name: "Mt. Haruna", category: "Street / Touge", country: "Japan" },
    TrackEntry { id: "isle_of_man", name: "Isle of Man TT", category: "Street / Touge", country: "UK" },
];

// ─── Featured Cars ───────────────────────────────────────────────────────────

const FEATURED_CARS: &[CarEntry] = &[
    // F1 2025
    CarEntry { id: "ferrari_sf25", name: "Ferrari SF-25", category: "F1 2025" },
    CarEntry { id: "red_bull_rb21", name: "Red Bull RB21", category: "F1 2025" },
    CarEntry { id: "mclaren_mcl39", name: "McLaren MCL39", category: "F1 2025" },
    CarEntry { id: "mercedes_w16", name: "Mercedes W16", category: "F1 2025" },
    CarEntry { id: "aston_martin_amr25", name: "Aston Martin AMR25", category: "F1 2025" },
    CarEntry { id: "williams_fw47", name: "Williams FW47", category: "F1 2025" },
    CarEntry { id: "racingbulls_rb02", name: "Racing Bulls VCARB 02", category: "F1 2025" },
    CarEntry { id: "gp_2025_a525", name: "Alpine A525", category: "F1 2025" },
    CarEntry { id: "gp_2025_c45", name: "Sauber C45", category: "F1 2025" },
    CarEntry { id: "gp_2025_vf25", name: "Haas VF-25", category: "F1 2025" },
    // GT3
    CarEntry { id: "ks_ferrari_488_gt3", name: "Ferrari 488 GT3", category: "GT3" },
    CarEntry { id: "cf_ferrari_296_gt3", name: "Ferrari 296 GT3", category: "GT3" },
    CarEntry { id: "ks_lamborghini_huracan_gt3", name: "Lamborghini Huracan GT3", category: "GT3" },
    CarEntry { id: "ks_mercedes_amg_gt3", name: "Mercedes AMG GT3", category: "GT3" },
    CarEntry { id: "ks_audi_r8_lms_2016", name: "Audi R8 LMS 2016", category: "GT3" },
    CarEntry { id: "ks_porsche_911_gt3_r_2016", name: "Porsche 911 GT3 R", category: "GT3" },
    CarEntry { id: "ks_mclaren_650_gt3", name: "McLaren 650S GT3", category: "GT3" },
    CarEntry { id: "ks_nissan_gtr_gt3", name: "Nissan GT-R GT3", category: "GT3" },
    CarEntry { id: "bmw_z4_gt3", name: "BMW Z4 GT3", category: "GT3" },
    // Supercars
    CarEntry { id: "ks_lamborghini_aventador_sv", name: "Lamborghini Aventador SV", category: "Supercars" },
    CarEntry { id: "ks_mclaren_p1", name: "McLaren P1", category: "Supercars" },
    CarEntry { id: "ferrari_laferrari", name: "Ferrari LaFerrari", category: "Supercars" },
    CarEntry { id: "ks_porsche_918_spyder", name: "Porsche 918 Spyder", category: "Supercars" },
    CarEntry { id: "bugatti_chiron", name: "Bugatti Chiron", category: "Supercars" },
    CarEntry { id: "koenigsegg_one", name: "Koenigsegg One:1", category: "Supercars" },
    CarEntry { id: "pagani_huayra", name: "Pagani Huayra", category: "Supercars" },
    CarEntry { id: "ks_ferrari_fxx_k", name: "Ferrari FXX K", category: "Supercars" },
    // Porsche
    CarEntry { id: "cky_porsche992_gt3rs_2023", name: "Porsche 992 GT3 RS", category: "Porsche" },
    CarEntry { id: "ks_porsche_911_gt3_rs", name: "Porsche 911 GT3 RS", category: "Porsche" },
    CarEntry { id: "ks_porsche_911_r", name: "Porsche 911 R", category: "Porsche" },
    CarEntry { id: "ks_porsche_991_turbo_s", name: "Porsche 991 Turbo S", category: "Porsche" },
    // JDM
    CarEntry { id: "ks_toyota_supra_mkiv", name: "Toyota Supra MK4", category: "JDM" },
    CarEntry { id: "ks_nissan_skyline_r34", name: "Nissan Skyline R34 GT-R", category: "JDM" },
    CarEntry { id: "ks_mazda_rx7_spirit_r", name: "Mazda RX-7 Spirit R", category: "JDM" },
    CarEntry { id: "ks_toyota_ae86", name: "Toyota AE86", category: "JDM" },
    CarEntry { id: "ks_nissan_gtr", name: "Nissan GT-R R35", category: "JDM" },
    // Classics / Fun
    CarEntry { id: "ks_ferrari_f2004", name: "Ferrari F2004", category: "Classics" },
    CarEntry { id: "ks_ferrari_250_gto", name: "Ferrari 250 GTO", category: "Classics" },
    CarEntry { id: "shelby_cobra_427sc", name: "Shelby Cobra 427 SC", category: "Classics" },
    CarEntry { id: "ks_ford_gt40", name: "Ford GT40", category: "Classics" },
    CarEntry { id: "ks_mazda_787b", name: "Mazda 787B", category: "Classics" },
];

// ─── All Track IDs ───────────────────────────────────────────────────────────
// Auto-populated from Pod 8 filesystem. Display names derived from folder ID.

const ALL_TRACK_IDS: &[&str] = &[
    "ADT", "acu_yasmarina", "albert-park_acu", "bahrain", "baku_2022",
    "china-gp", "cota", "daytona_2017", "drift", "fn_losail",
    "haruna", "imola", "india", "interlagos", "isle_of_man",
    "jeddah21", "kari_motor_speedway", "ks_barcelona", "ks_black_cat_county",
    "ks_brands_hatch", "ks_drag", "ks_highlands", "ks_laguna_seca",
    "ks_monza66", "ks_nordschleife", "ks_nurburgring", "ks_red_bull_ring",
    "ks_silverstone", "ks_silverstone1967", "ks_vallelunga", "ks_zandvoort",
    "lasvegas23", "madras_international_circuit", "magione", "monaco", "monza",
    "mugello", "phillip_island_2013", "phillip_island_circuit", "rt_suzuka",
    "shibuya-hachiko drift", "shuto_revival_project_beta", "singapore", "spa",
    "spielberg", "sx_lemans", "trento-bondone", "vrc_mexico",
    "yas_marina_circuit-day", "yas_marina_circuit-night",
];

// ─── All Car IDs ─────────────────────────────────────────────────────────────

const ALL_CAR_IDS: &[&str] = &[
    "660_series_ha23v_ce28", "ApexGP", "Gravel_Mitsubishi_Evo9_R4", "PurSport",
    "abarth500", "abarth500_s1",
    "acme_hyundai_i20_rally1_22", "acra_suzuki_swift_proto2",
    "aegis_mitsubishi_lancer_evolution_v_gsr",
    "alfa_romeo_giulietta_qv", "alfa_romeo_giulietta_qv_le",
    "alm_supra_a60", "amy_ek_cup", "amy_honda_dc2_turbo",
    "amy_honda_ek9_turbo", "arch_ruf_ctr_1987",
    "art_diablo_gtr", "art_mazda_fd3s_rx7_black_eagle",
    "art_nissan_gtr_bcnr33_600r", "art_porsche_911_gt3_996", "art_skyline_r32_gtr",
    "aston_martin_amr25", "aston_martin_valkyrie_amr_pro_2022",
    "bati_e46_nspec54", "bati_e46_nspec85", "bati_fd3s_rx7",
    "bksy_nissan_skyline_r34_vspec", "bksy_nissan_skyline_r34_vspec_ii_nur",
    "bmw_1m", "bmw_1m_s3", "bmw_m3_e30", "bmw_m3_e30_drift",
    "bmw_m3_e30_dtm", "bmw_m3_e30_gra", "bmw_m3_e30_s1",
    "bmw_m3_e92", "bmw_m3_e92_drift", "bmw_m3_e92_s1",
    "bmw_m3_gt2", "bmw_m8_LMC", "bmw_z4", "bmw_z4_drift",
    "bmw_z4_gt3", "bmw_z4_s1", "bugatti_chiron",
    "cf_ferrari_296_gt3", "cky_lamborghini_revuelto",
    "cky_porsche992_gt3rs_2023",
    "ddm_daihatsu_copen_street", "ddm_honda_civic_fd2",
    "ddm_honda_s2000_ap1", "ddm_mazda_fc3s_re",
    "ddm_mazda_rx7_infini_fc3s", "ddm_mitsubishi_evo_iv_gsr",
    "ddm_mugen_civic_aero_ek9", "ddm_nissan_silvia_s14k",
    "ddm_nissan_silvia_s14k_opt", "ddm_nissan_silvia_s15",
    "ddm_nissan_skyline_bnr32", "ddm_nissan_skyline_hr31_house",
    "ddm_subaru_22b", "ddm_toyota_mr2_sw20",
    "ddm_toyota_mr2_sw20_shuto", "ddm_toyota_mrs_c_one",
    "ddm_toyota_mrs_haru", "ddm_toyota_supra_ma70",
    "ecf_lotus_europa_wolf",
    "exmods_mercedes_amg_gt_coupe24",
    "f1_1986_mclaren", "f1_2020_mercedes",
    "ferrari_296_gts", "ferrari_312t", "ferrari_458",
    "ferrari_458_gt2", "ferrari_458_s3", "ferrari_599xxevo",
    "ferrari_f40", "ferrari_f40_s3", "ferrari_laferrari", "ferrari_sf25",
    "gmp_abflug_s900", "gmp_e60_m5_f10style", "gmp_jzs161_ridox",
    "gp_2025_a525", "gp_2025_amr25", "gp_2025_c45",
    "gp_2025_fw47", "gp_2025_mcl39", "gp_2025_rb21",
    "gp_2025_sf25", "gp_2025_vcarb02", "gp_2025_vf25", "gp_2025_w16",
    "gt4_toyota_supra",
    "honda_acty_ha3", "hsrc_subaru_gc8",
    "j8_ae86_tuned_coupe", "j8_eunos_roadster_tuned",
    "j8_mitsubishi_gto_twin_turbo_91", "j8_mitsubishi_gto_twin_turbo_91_haru_spec",
    "j8_toyota_celica_tuned", "j8_toyota_mr2_sw20",
    "koenigsegg_one", "koenigsegg_one_nr", "koenigsegg_one_p", "koenigsegg_one_t",
    "ks_abarth500_assetto_corse", "ks_abarth_595ss",
    "ks_abarth_595ss_s1", "ks_abarth_595ss_s2",
    "ks_alfa_33_stradale", "ks_alfa_giulia_qv",
    "ks_alfa_mito_qv", "ks_alfa_romeo_155_v6",
    "ks_alfa_romeo_4c", "ks_alfa_romeo_gta",
    "ks_audi_a1s1", "ks_audi_r18_etron_quattro",
    "ks_audi_r8_lms", "ks_audi_r8_lms_2016",
    "ks_audi_r8_plus", "ks_audi_sport_quattro",
    "ks_audi_sport_quattro_rally", "ks_audi_sport_quattro_s1",
    "ks_audi_tt_cup", "ks_audi_tt_vln",
    "ks_bmw_m235i_racing", "ks_bmw_m4", "ks_bmw_m4_akrapovic",
    "ks_corvette_c7_stingray", "ks_corvette_c7r",
    "ks_ferrari_250_gto", "ks_ferrari_288_gto",
    "ks_ferrari_312_67", "ks_ferrari_330_p4",
    "ks_ferrari_488_challenge_evo", "ks_ferrari_488_gt3",
    "ks_ferrari_488_gt3_2020", "ks_ferrari_488_gtb",
    "ks_ferrari_812_superfast", "ks_ferrari_f138",
    "ks_ferrari_f2004", "ks_ferrari_fxx_k",
    "ks_ferrari_sf15t", "ks_ferrari_sf70h",
    "ks_ford_escort_mk1", "ks_ford_gt40",
    "ks_ford_mustang_2015", "ks_glickenhaus_scg003",
    "ks_lamborghini_aventador_sv", "ks_lamborghini_countach",
    "ks_lamborghini_countach_s1", "ks_lamborghini_gallardo_sl",
    "ks_lamborghini_gallardo_sl_s3", "ks_lamborghini_huracan_gt3",
    "ks_lamborghini_huracan_performante", "ks_lamborghini_huracan_st",
    "ks_lamborghini_miura_sv", "ks_lamborghini_sesto_elemento",
    "ks_lotus_25", "ks_lotus_3_eleven", "ks_lotus_72d",
    "ks_maserati_250f_12cyl", "ks_maserati_250f_6cyl",
    "ks_maserati_alfieri", "ks_maserati_gt_mc_gt4",
    "ks_maserati_levante", "ks_maserati_mc12_gt1",
    "ks_maserati_quattroporte",
    "ks_mazda_787b", "ks_mazda_miata", "ks_mazda_mx5_cup",
    "ks_mazda_mx5_nd", "ks_mazda_rx7_spirit_r", "ks_mazda_rx7_tuned",
    "ks_mclaren_570s", "ks_mclaren_650_gt3",
    "ks_mclaren_f1_gtr", "ks_mclaren_p1", "ks_mclaren_p1_gtr",
    "ks_mercedes_190_evo2", "ks_mercedes_amg_gt3", "ks_mercedes_c9",
    "ks_nissan_370z", "ks_nissan_gtr", "ks_nissan_gtr_gt3",
    "ks_nissan_skyline_r34",
    "ks_pagani_huayra_bc",
    "ks_porsche_718_boxster_s", "ks_porsche_718_boxster_s_pdk",
    "ks_porsche_718_cayman_s", "ks_porsche_718_spyder_rs",
    "ks_porsche_908_lh", "ks_porsche_911_carrera_rsr",
    "ks_porsche_911_gt1", "ks_porsche_911_gt3_cup_2017",
    "ks_porsche_911_gt3_r_2016", "ks_porsche_911_gt3_rs",
    "ks_porsche_911_r", "ks_porsche_911_rsr_2017",
    "ks_porsche_917_30", "ks_porsche_917_k",
    "ks_porsche_918_spyder", "ks_porsche_919_hybrid_2015",
    "ks_porsche_919_hybrid_2016", "ks_porsche_935_78_moby_dick",
    "ks_porsche_962c_longtail", "ks_porsche_962c_shorttail",
    "ks_porsche_991_carrera_s", "ks_porsche_991_turbo_s",
    "ks_porsche_cayenne", "ks_porsche_cayman_gt4_clubsport",
    "ks_porsche_cayman_gt4_std", "ks_porsche_macan", "ks_porsche_panamera",
    "ks_praga_r1", "ks_ruf_rt12r", "ks_ruf_rt12r_awd",
    "ks_toyota_ae86", "ks_toyota_ae86_drift", "ks_toyota_ae86_tuned",
    "ks_toyota_celica_st185", "ks_toyota_gt86",
    "ks_toyota_supra_mkiv", "ks_toyota_supra_mkiv_drift",
    "ks_toyota_supra_mkiv_tuned", "ks_toyota_ts040",
    "ktm_xbow_r",
    "lk_nissan_180sx_96",
    "lotus_2_eleven", "lotus_2_eleven_gt4", "lotus_49", "lotus_98t",
    "lotus_elise_sc", "lotus_elise_sc_s1", "lotus_elise_sc_s2",
    "lotus_evora_gtc", "lotus_evora_gte", "lotus_evora_gte_carbon",
    "lotus_evora_gx", "lotus_evora_s", "lotus_evora_s_s2",
    "lotus_exige_240", "lotus_exige_240_s3", "lotus_exige_s",
    "lotus_exige_s_roadster", "lotus_exige_scura", "lotus_exige_v6_cup",
    "lotus_exos_125", "lotus_exos_125_s1",
    "ltkaeri_honda_s2000_gt1_amuse",
    "mclaren_mcl38", "mclaren_mcl39",
    "mclaren_mp412c", "mclaren_mp412c_gt3",
    "mercedes_g65_amg", "mercedes_sls", "mercedes_sls_gt3", "mercedes_w16",
    "naz_jza80_ridox_modern", "naz_porsche_924_tuned",
    "nissan_skyline_r34_omori_factory_s1", "nissan_skyline_r34_v-specperformance",
    "nohesi_bmw_m2_f87_comp", "nohesi_bmw_m3_e92_adro",
    "nohesi_chevrolet_corvette_c6", "nohesi_g82_comp_coupe",
    "nohesi_lamborghini_huracan_lp610", "nohesi_lamborghini_urus_performante_vlct",
    "nohesi_lexus_lfa_nurburgring", "nohesi_mclaren_600lt_novitec",
    "nohesi_mclaren_720s_hjckd", "nohesi_mercedes_brabus_gt600",
    "nohesi_mercedes_gt63", "nohesi_realistic_audi_rs3_saloon_vlct",
    "nohesi_skyline_r34", "nohesi_toyota_supra_mk4",
    "nohesituned_dodge_viper",
    "ohyeah2389_modkart_dd2", "ohyeah2389_modkart_ka100sr",
    "ohyeah2389_modkart_lo206", "ohyeah2389_modkart_rokshifter",
    "p3_mitsubishi_evo8", "p4-5_2011",
    "pagani_huayra", "pagani_zonda_r",
    "pear_nissan_silvia_s13_wangan",
    "racingbulls_rb02", "red_bull_rb21", "red_bull_rb21_s2",
    "rize_efini_rx7_fd3s_keisuke_1", "rize_ferrari_f355_challenge_persephone",
    "rj_honda_civic_eg6_tuned", "rkm_landrover_def",
    "rss_formula_2000", "rss_formula_2010",
    "rss_formula_hybrid_2025_alpine", "rss_formula_rss_supreme_25",
    "rss_gtm_bayer_v8", "rss_gtm_furiano_96_v6",
    "rss_gtm_hyperion_v8", "rss_gtm_mercer_v8", "rss_gtm_protech_p92_f6",
    "ruf_yellowbird", "shelby_cobra_427sc",
    "sl_toyota_supra_mkiv_ridox", "slang_ferrari_f40",
    "snp_zhonghua_zidantou_wangan_spec",
    "spear_lamborghini_lp640_veilside",
    "srp_bcnr33_wangan", "srp_honda_s2000_legendary",
    "srp_mitsubishi_evo_5_kai", "srp_toyota_supra_mkiv_interceptor",
    "ste_urus",
    "sts_e60_m5", "sts_e60_m5_ericsson", "sts_e60_m5_manual",
    "tatuusfa1", "tmm_skoda_130rs_hillclimb",
    "toyota_fortuner_2021_legender", "williams_fw47",
    "wm_mazda_rx7_fd_rgo", "wm_nissan_fairlady_z_s30",
    "wm_nissan_s15", "wm_porsche_911_930",
];

/// Derive a human-readable name from a folder ID
fn id_to_display_name(id: &str) -> String {
    // Strip common prefixes
    let stripped = id
        .strip_prefix("ks_")
        .or_else(|| id.strip_prefix("ddm_"))
        .or_else(|| id.strip_prefix("gp_2025_"))
        .or_else(|| id.strip_prefix("rss_"))
        .or_else(|| id.strip_prefix("nohesi_"))
        .or_else(|| id.strip_prefix("art_"))
        .or_else(|| id.strip_prefix("srp_"))
        .or_else(|| id.strip_prefix("j8_"))
        .or_else(|| id.strip_prefix("wm_"))
        .unwrap_or(id);

    stripped
        .replace('_', " ")
        .replace('-', " ")
        .split_whitespace()
        .map(|word| {
            // Keep ALL-CAPS words (e.g. "GT3", "BMW", "LMS")
            if word.len() <= 4 && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
                word.to_string()
            } else {
                // Title case
                let mut chars = word.chars();
                match chars.next() {
                    Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build the full catalog JSON
pub fn get_catalog() -> Value {
    let featured_tracks: Vec<Value> = FEATURED_TRACKS
        .iter()
        .map(|t| json!({ "id": t.id, "name": t.name, "category": t.category, "country": t.country }))
        .collect();

    let all_tracks: Vec<Value> = ALL_TRACK_IDS
        .iter()
        .map(|id| {
            // Use featured name if available, otherwise derive
            let featured = FEATURED_TRACKS.iter().find(|t| t.id == *id);
            match featured {
                Some(t) => json!({ "id": t.id, "name": t.name, "category": t.category, "country": t.country }),
                None => json!({ "id": id, "name": id_to_display_name(id), "category": "Other", "country": "" }),
            }
        })
        .collect();

    let featured_cars: Vec<Value> = FEATURED_CARS
        .iter()
        .map(|c| json!({ "id": c.id, "name": c.name, "category": c.category }))
        .collect();

    let all_cars: Vec<Value> = ALL_CAR_IDS
        .iter()
        .map(|id| {
            let featured = FEATURED_CARS.iter().find(|c| c.id == *id);
            match featured {
                Some(c) => json!({ "id": c.id, "name": c.name, "category": c.category }),
                None => json!({ "id": id, "name": id_to_display_name(id), "category": "Other" }),
            }
        })
        .collect();

    json!({
        "tracks": {
            "featured": featured_tracks,
            "all": all_tracks,
        },
        "cars": {
            "featured": featured_cars,
            "all": all_cars,
        },
        "categories": {
            "tracks": ["F1 Circuits", "Real Circuits", "Indian Circuits", "Street / Touge", "Other"],
            "cars": ["F1 2025", "GT3", "Supercars", "Porsche", "JDM", "Classics", "Other"],
        }
    })
}

// ─── Difficulty Presets ──────────────────────────────────────────────────────

/// Build launch_args JSON with difficulty/transmission/conditions encoded
pub fn build_custom_launch_args(
    car: &str,
    track: &str,
    driver: &str,
    difficulty: &str,
    transmission: &str,
) -> Value {
    let (abs, tc, stability, autoclutch, ideal_line) = match difficulty {
        "easy" => (1, 1, 1, 1, 1),
        "medium" => (1, 1, 0, 1, 0),
        "hard" => (0, 0, 0, 0, 0),
        _ => (1, 1, 1, 1, 1), // default to easy
    };

    json!({
        "car": car,
        "track": track,
        "driver": driver,
        "difficulty": difficulty,
        "transmission": transmission,
        "aids": {
            "abs": abs,
            "tc": tc,
            "stability": stability,
            "autoclutch": autoclutch,
            "ideal_line": ideal_line,
        },
        "conditions": {
            "damage": 0,
            "dynamic_track": {
                "session_start": 100,
                "randomness": 0,
                "session_transfer": 100,
                "lap_gain": 0,
            },
            "sun_angle": 16,
            "weather": "clear",
        }
    })
}
