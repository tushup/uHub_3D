use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetType {
    Model,
    Material,
    Hdri,
}

impl AssetType {
    pub fn folder(&self) -> &'static str {
        match self {
            AssetType::Model => "3D",
            AssetType::Material => "Materials",
            AssetType::Hdri => "HDRI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttrKind {
    BaseColor,
    Metallic,
    Roughness,
    Specular,
    Normal,
    Height,
    AO,
    Opacity,
    Emissive,
}

impl AttrKind {
    pub fn is_srgb(&self) -> bool {
        matches!(self, AttrKind::BaseColor | AttrKind::Emissive)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalY {
    OpenGL,
    DirectX,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttrValue {
    Scalar(f32),
    Color([f32; 3]),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribute {
    pub kind: AttrKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<AttrValue>,
    #[serde(default)]
    pub texture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_y: Option<NormalY>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub original_name: String,
    pub name: String,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub file: String,
    pub format: String,
    pub meshes: u32,
    pub vertices: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HdriInfo {
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub rotation: f32,
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub schema: u32,
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: AssetType,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub created: String,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub model: Option<ModelInfo>,
    pub materials: Vec<Material>,
    #[serde(default)]
    pub hdri: Option<HdriInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<crate::view::View>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeDraft {
    pub kind: AttrKind,
    #[serde(default)]
    pub value: Option<AttrValue>,
    #[serde(default)]
    pub texture_src: Option<PathBuf>,
    #[serde(default)]
    pub strength: Option<f32>,
    #[serde(default)]
    pub normal_y: Option<NormalY>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDraft {
    pub original_name: String,
    pub name: String,
    pub attributes: Vec<AttributeDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDraft {
    pub asset_type: AssetType,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub model_src: Option<PathBuf>,
    #[serde(default)]
    pub hdri_src: Option<PathBuf>,
    #[serde(default)]
    pub thumbnail_src: Option<PathBuf>,
    pub materials: Vec<MaterialDraft>,
    pub rotation: f32,
    pub intensity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<crate::view::View>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_json_roundtrip() {
        let asset = Asset {
            schema: 1,
            id: "abc".into(),
            r#type: AssetType::Model,
            name: "Chair".into(),
            category: "Furniture".into(),
            tags: vec!["wood".into()],
            created: "2026-10-30T10:00:00Z".into(),
            thumbnail: Some("thumbnail.png".into()),
            model: Some(ModelInfo {
                file: "Chair.fbx".into(),
                format: "fbx".into(),
                meshes: 3,
                vertices: 5120,
            }),
            view: None,
            materials: vec![Material {
                original_name: "lambert3".into(),
                name: "Seat".into(),
                attributes: vec![
                    Attribute {
                        kind: AttrKind::BaseColor,
                        value: Some(AttrValue::Color([0.8, 0.8, 0.8])),
                        texture: Some("Chair_Seat_BaseColor.png".into()),
                        strength: None,
                        normal_y: None,
                    },
                    Attribute {
                        kind: AttrKind::Normal,
                        value: None,
                        texture: Some("Chair_Seat_Normal.png".into()),
                        strength: Some(1.0),
                        normal_y: Some(NormalY::DirectX),
                    },
                ],
            }],
            hdri: None,
        };
        let json = serde_json::to_string(&asset).unwrap();
        assert!(json.contains("\"type\":\"model\""), "{json}");
        assert!(json.contains("\"originalName\""), "{json}");
        assert!(json.contains("\"normalY\":\"DirectX\""), "{json}");
        assert!(json.contains("\"value\":[0.8,0.8,0.8]"), "{json}");
        let back: Asset = serde_json::from_str(&json).unwrap();
        assert_eq!(back, asset);
    }

    #[test]
    fn asset_json_without_view_loads() {
        // M1-era file: no "view" key at all.
        let json = r#"{
            "schema": 1, "id": "x", "type": "hdri", "name": "Sky",
            "category": "Skies", "tags": [], "created": "2026-10-30T00:00:00Z",
            "materials": [],
            "hdri": { "file": "Sky.hdr", "width": 512, "height": 256, "rotation": 0.0, "intensity": 1.0 }
        }"#;
        let asset: Asset = serde_json::from_str(json).unwrap();
        assert_eq!(asset.view, None);
        // And a draft without view parses, and stays absent when serialized.
        let draft: AssetDraft = serde_json::from_str(
            r#"{ "assetType": "hdri", "name": "Sky", "category": "Skies", "tags": [],
                 "materials": [], "rotation": 0.0, "intensity": 1.0 }"#,
        )
        .unwrap();
        assert_eq!(draft.view, None);
        let with = serde_json::to_string(&AssetDraft {
            view: Some(crate::view::View::default()),
            ..draft
        })
        .unwrap();
        assert!(with.contains("\"view\":{\"yaw\":-30.0,\"pitch\":15.0,\"zoom\":1.0}"), "{with}");
    }

    #[test]
    fn srgb_kinds() {
        assert!(AttrKind::BaseColor.is_srgb() && AttrKind::Emissive.is_srgb() && !AttrKind::Normal.is_srgb());
    }
}
