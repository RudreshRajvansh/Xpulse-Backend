use crate::models::{LabOrder, Medicine, PatientProfile, Recommendation};

pub fn build(profile: &PatientProfile, medicines: &[Medicine], labs: &[LabOrder]) -> Vec<Recommendation> {
    let mut out: Vec<Recommendation> = Vec::new();

    let bmi = bmi_of(profile);
    let has = |needle: &str| {
        profile
            .conditions
            .iter()
            .any(|c| c.eq_ignore_ascii_case(needle))
    };

    if has("Diabetes") {
        out.push(Recommendation {
            title: "Endocrinology review".into(),
            specialty: "Endocrinologist".into(),
            reason: "Diabetes is listed in your medical history. A quarterly HbA1c review is standard care.".into(),
            urgency: "ROUTINE".into(),
        });
    }

    if has("Hypertension") || has("Heart Disease") {
        out.push(Recommendation {
            title: "Cardiology check-up".into(),
            specialty: "Cardiologist".into(),
            reason: "Your history includes a cardiovascular condition. Regular blood pressure and lipid monitoring is advised.".into(),
            urgency: "ROUTINE".into(),
        });
    }

    if has("Asthma") {
        out.push(Recommendation {
            title: "Pulmonology review".into(),
            specialty: "Pulmonologist".into(),
            reason: "Asthma is listed in your history. Inhaler technique and control should be reviewed periodically.".into(),
            urgency: "ROUTINE".into(),
        });
    }

    if has("Thyroid") {
        out.push(Recommendation {
            title: "Thyroid function test".into(),
            specialty: "Endocrinologist".into(),
            reason: "Thyroid condition on record. TSH levels are usually rechecked every 6 to 12 months.".into(),
            urgency: "ROUTINE".into(),
        });
    }

    if let Some(value) = bmi {
        if value >= 30.0 {
            out.push(Recommendation {
                title: "Weight and nutrition consult".into(),
                specialty: "Dietitian".into(),
                reason: format!("Your BMI is {value:.1}, which falls in the obese range. Diet and activity planning can reduce cardiometabolic risk."),
                urgency: "ROUTINE".into(),
            });
        } else if value >= 25.0 {
            out.push(Recommendation {
                title: "Lifestyle and diet review".into(),
                specialty: "Dietitian".into(),
                reason: format!("Your BMI is {value:.1}, slightly above the healthy range for adults."),
                urgency: "LOW".into(),
            });
        } else if value < 18.5 {
            out.push(Recommendation {
                title: "Nutrition assessment".into(),
                specialty: "Dietitian".into(),
                reason: format!("Your BMI is {value:.1}, below the healthy range. Underlying causes are worth ruling out."),
                urgency: "ROUTINE".into(),
            });
        }
    }

    let pending_labs = labs.iter().filter(|l| l.status != "COMPLETED").count();
    if pending_labs > 0 {
        out.push(Recommendation {
            title: format!("{pending_labs} lab test(s) still pending"),
            specialty: "Diagnostics".into(),
            reason: "Completing ordered tests lets your doctor act on current results.".into(),
            urgency: "ACTION".into(),
        });
    }

    if medicines.len() >= 5 {
        out.push(Recommendation {
            title: "Medication review".into(),
            specialty: "General Physician".into(),
            reason: format!("You are on {} medicines. A review reduces the risk of interactions.", medicines.len()),
            urgency: "ROUTINE".into(),
        });
    }

    if profile.age.parse::<u32>().map(|a| a >= 45).unwrap_or(false) && !has("None") {
        out.push(Recommendation {
            title: "Annual preventive screening".into(),
            specialty: "General Physician".into(),
            reason: "Adults over 45 benefit from yearly blood pressure, sugar and lipid screening.".into(),
            urgency: "LOW".into(),
        });
    }

    if out.is_empty() {
        out.push(Recommendation {
            title: "No follow-up flagged".into(),
            specialty: "General Physician".into(),
            reason: "Nothing in your record triggers a recommendation right now. Keep your profile up to date.".into(),
            urgency: "NONE".into(),
        });
    }

    out
}

fn bmi_of(profile: &PatientProfile) -> Option<f32> {
    let h = profile.height_cm.parse::<f32>().ok()? / 100.0;
    let w = profile.weight_kg.parse::<f32>().ok()?;
    if h <= 0.0 { None } else { Some(w / (h * h)) }
}
