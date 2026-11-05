fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    config.out_dir("src/shared/generated");
    config.file_descriptor_set_path("proto/autonomy.binpb");
    config.compile_protos(&["proto/autonomy.proto"], &["proto"])?;
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let raw = std::fs::read("proto/autonomy.binpb")?;
    let mut profile: serde_json::Value =
        serde_json::from_slice(&std::fs::read("config/protobuf-profile.json")?)?;
    profile["descriptor"] =
        serde_json::json!(base64::engine::general_purpose::STANDARD.encode(&raw));
    profile["fingerprint"] = serde_json::json!(format!("{:x}", Sha256::digest(raw)));
    std::fs::write(
        "config/protobuf-profile.json",
        serde_json::to_string_pretty(&profile)? + "\n",
    )?;
    Ok(())
}
