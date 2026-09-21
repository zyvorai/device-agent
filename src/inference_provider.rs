// SPDX-License-Identifier: Apache-2.0

//! Inference provider contract. Device Agent supervises providers and publishes
//! status. It does not download models, run them, or remediate from them.
//! Model delivery belongs to Zyvor OTA / Fleet.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    OnnxCpu,
    OpenVino,
    Rknn,
    TensorRt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider: String,
    pub state: &'static str,
    pub executes_models: bool,
    pub may_remediate: bool,
}

pub trait InferenceProvider {
    fn kind(&self) -> ProviderKind;
    fn status(&self) -> ProviderStatus;
}

pub struct OnnxCpu;
pub struct OpenVino;
pub struct Rknn;
pub struct TensorRt;

impl InferenceProvider for OnnxCpu {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OnnxCpu
    }
    fn status(&self) -> ProviderStatus {
        not_configured("onnx-cpu")
    }
}

impl InferenceProvider for OpenVino {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenVino
    }
    fn status(&self) -> ProviderStatus {
        not_configured("openvino")
    }
}

impl InferenceProvider for Rknn {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Rknn
    }
    fn status(&self) -> ProviderStatus {
        not_configured("rknn")
    }
}

impl InferenceProvider for TensorRt {
    fn kind(&self) -> ProviderKind {
        ProviderKind::TensorRt
    }
    fn status(&self) -> ProviderStatus {
        not_configured("tensorrt")
    }
}

fn not_configured(name: &str) -> ProviderStatus {
    ProviderStatus {
        provider: name.into(),
        state: "not-configured",
        executes_models: false,
        may_remediate: false,
    }
}

pub fn all_providers() -> Vec<ProviderStatus> {
    vec![
        OnnxCpu.status(),
        OpenVino.status(),
        Rknn.status(),
        TensorRt.status(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_never_execute_or_remediate() {
        for status in all_providers() {
            assert_eq!(status.state, "not-configured");
            assert!(!status.executes_models);
            assert!(!status.may_remediate);
        }
    }
}
