use crate::store::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSettings {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffortOption {
    pub reasoning_effort: String,
    pub description: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexModel {
    pub model: String,
    pub display_name: String,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<EffortOption>,
    pub is_default: bool,
    #[serde(default)]
    pub hidden: bool,
}

pub fn validate(models: &[CodexModel], settings: &ModelSettings) -> Result<()> {
    let Some(id) = settings.model.as_deref() else {
        return if settings.reasoning_effort.is_none() { Ok(()) } else { Err("모델을 먼저 선택해줘".into()) };
    };
    let model = models.iter().find(|m| m.model == id && !m.hidden)
        .ok_or("현재 Codex 목록에 없는 모델이야. 목록을 새로 불러와줘.")?;
    let valid = settings.reasoning_effort.as_deref().is_some_and(|effort| {
        model.supported_reasoning_efforts.iter().any(|e| e.reasoning_effort == effort)
            || (model.supported_reasoning_efforts.is_empty() && effort == model.default_reasoning_effort)
    });
    if !valid { return Err("이 모델이 지원하지 않는 추론 강도야. 다시 선택해줘.".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_models_and_model_specific_efforts() {
        let models = vec![CodexModel { model: "model-a".into(), display_name: "A".into(), default_reasoning_effort: "medium".into(), supported_reasoning_efforts: vec![EffortOption { reasoning_effort: "medium".into(), description: "".into() }], is_default: true, hidden: false }];
        assert!(validate(&models, &ModelSettings::default()).is_ok());
        for (model, effort, valid) in [("model-a", "medium", true), ("model-a", "ultra", false), ("not-listed", "medium", false)] {
            assert_eq!(validate(&models, &ModelSettings { model: Some(model.into()), reasoning_effort: Some(effort.into()) }).is_ok(), valid);
        }
    }
}
