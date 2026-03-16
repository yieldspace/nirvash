use std::{
    collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestFilter {
    pub spec: Option<String>,
    pub binding: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeRequest {
    pub spec: String,
    pub binding: String,
    pub profile: String,
    pub replay: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManifestRecord {
    pub spec: String,
    pub spec_slug: String,
    pub spec_path: String,
    pub export_module: String,
    pub crate_package: String,
    pub crate_manifest_dir: String,
    pub default_profiles: Vec<String>,
    pub binding: String,
    pub profile: String,
    pub engine: Value,
    pub coverage: Vec<Value>,
    pub replay_dir: Option<String>,
    pub materialize_failures: bool,
}

#[derive(Debug, Deserialize)]
struct RawManifestRecord {
    #[serde(alias = "spec_name")]
    spec: String,
    #[serde(default)]
    spec_slug: String,
    #[serde(default)]
    spec_path: String,
    export_module: String,
    #[serde(default)]
    crate_package: String,
    #[serde(default)]
    crate_manifest_dir: String,
    #[serde(default)]
    default_profiles: Vec<String>,
    #[serde(default)]
    binding: Option<String>,
    #[serde(default)]
    binding_path: Option<String>,
    profile: String,
    #[serde(default, alias = "engines")]
    engine: Value,
    #[serde(default)]
    coverage: Vec<Value>,
    #[serde(default)]
    replay_dir: Option<String>,
    #[serde(default)]
    materialize_failures: bool,
}

impl<'de> Deserialize<'de> for ManifestRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawManifestRecord::deserialize(deserializer)?;
        let binding = match (raw.binding, raw.binding_path) {
            (Some(binding), Some(binding_path)) if binding == binding_path => binding,
            (Some(_), Some(_)) => {
                return Err(serde::de::Error::custom(
                    "manifest fields `binding` and `binding_path` disagree",
                ));
            }
            (Some(binding), None) | (None, Some(binding)) => binding,
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "manifest must contain `binding` or `binding_path`",
                ));
            }
        };
        let binding = normalize_binding_path(&binding);

        Ok(Self {
            spec: raw.spec,
            spec_slug: raw.spec_slug,
            spec_path: raw.spec_path,
            export_module: raw.export_module,
            crate_package: raw.crate_package,
            crate_manifest_dir: raw.crate_manifest_dir,
            default_profiles: raw.default_profiles,
            binding,
            profile: raw.profile,
            engine: raw.engine,
            coverage: raw.coverage,
            replay_dir: raw.replay_dir,
            materialize_failures: raw.materialize_failures,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProfileRecord {
    profile: String,
    engine: Value,
    coverage: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayBundleRecord {
    #[serde(alias = "spec_name")]
    pub spec: String,
    pub profile: String,
    pub engine: String,
    pub detail: Value,
    pub action_trace: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaySummary {
    pub json_path: PathBuf,
    pub ndjson_path: PathBuf,
    pub spec: String,
    pub profile: String,
    pub engine: String,
    pub event_count: usize,
}

pub fn target_nirvash_dir() -> PathBuf {
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    target_dir.join("nirvash")
}

pub fn list_tests(base: impl AsRef<Path>, filter: &ManifestFilter) -> Result<Vec<ManifestRecord>> {
    let mut manifests = Vec::new();
    let manifest_dir = base.as_ref().join("manifest");
    if !manifest_dir.exists() {
        return Ok(manifests);
    }

    let mut manifest_paths = read_json_files(&manifest_dir)?;
    manifest_paths.sort();
    for path in manifest_paths {
        let manifest: ManifestRecord = serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
        )
        .with_context(|| format!("failed to decode manifest {}", path.display()))?;
        manifests.push(manifest);
    }

    manifests = dedupe_manifests(manifests);
    manifests = augment_default_profiles(base.as_ref(), manifests, filter)?;
    manifests.retain(|manifest| manifest_matches(manifest, filter));
    manifests.sort_by(|lhs, rhs| {
        lhs.spec
            .cmp(&rhs.spec)
            .then(lhs.profile.cmp(&rhs.profile))
            .then(lhs.binding.cmp(&rhs.binding))
    });
    Ok(manifests)
}

pub fn materialize_tests(
    base: impl AsRef<Path>,
    request: &MaterializeRequest,
) -> Result<Vec<PathBuf>> {
    let base = base.as_ref();
    let manifests = list_tests(
        base,
        &ManifestFilter {
            spec: Some(request.spec.clone()),
            binding: Some(request.binding.clone()),
            profile: Some(request.profile.clone()),
        },
    )?;
    if manifests.is_empty() {
        bail!(
            "no manifest matched spec={} binding={} profile={}",
            request.spec,
            request.binding,
            request.profile
        );
    }

    let replay_override = request
        .replay
        .as_deref()
        .map(canonical_bundle_json_path)
        .transpose()?;
    let mut paths = Vec::new();
    for manifest in manifests {
        let materialized_dir = materialized_output_dir(base, &manifest);
        fs::create_dir_all(&materialized_dir)
            .with_context(|| format!("failed to create {}", materialized_dir.display()))?;
        let replay_path = if let Some(path) = replay_override.clone() {
            Some(path)
        } else {
            latest_matching_replay_bundle(base, &manifest).ok()
        };
        if let Some(replay_path) = replay_path {
            let output_path = materialized_dir.join(format!(
                "{}_{}_replay.rs",
                sanitize_path(manifest_spec_stem(&manifest)),
                sanitize_path(&manifest.profile),
            ));
            fs::write(
                &output_path,
                render_materialized_test(&manifest, &replay_path),
            )
            .with_context(|| format!("failed to write {}", output_path.display()))?;
            refresh_materialized_index(
                &materialized_dir,
                &materialized_index_path(base, &manifest),
            )?;
            paths.push(output_path);
        } else {
            bail!(
                "no replay bundle found for spec={} profile={}",
                manifest.spec,
                manifest.profile
            );
        }
    }

    Ok(paths)
}

pub fn materialize_replay(
    base: impl AsRef<Path>,
    replay_path: impl AsRef<Path>,
) -> Result<Vec<PathBuf>> {
    let base = base.as_ref();
    let replay_path = canonical_bundle_json_path(replay_path.as_ref())?;
    let bundle = read_replay_record(&replay_path)?;
    let manifests = list_tests(
        base,
        &ManifestFilter {
            spec: Some(bundle.spec.clone()),
            binding: None,
            profile: Some(bundle.profile.clone()),
        },
    )?;
    if manifests.is_empty() {
        bail!(
            "no manifest matched replay bundle spec={} profile={}",
            bundle.spec,
            bundle.profile
        );
    }
    if manifests.len() != 1 {
        bail!(
            "replay bundle matched multiple bindings for spec={} profile={}; rerun materialize-tests with --binding",
            bundle.spec,
            bundle.profile
        );
    }
    let manifest = manifests
        .into_iter()
        .next()
        .expect("manifest presence already checked");
    materialize_tests(
        base,
        &MaterializeRequest {
            spec: manifest.spec,
            binding: manifest.binding,
            profile: manifest.profile,
            replay: Some(replay_path),
        },
    )
}

pub fn replay(base: impl AsRef<Path>) -> Result<Vec<ReplaySummary>> {
    let replay_dir = base.as_ref().join("replay");
    if !replay_dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();
    for json_path in read_json_files(&replay_dir)? {
        let file_name = json_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !file_name.ends_with("_bundle.json") {
            continue;
        }

        let bundle = read_replay_record(&json_path)?;
        let ndjson_path = json_path.with_file_name(file_name.replace(".json", ".ndjson"));
        let event_count = validate_ndjson(&ndjson_path)?;
        summaries.push(ReplaySummary {
            json_path,
            ndjson_path,
            spec: bundle.spec,
            profile: bundle.profile,
            engine: bundle.engine,
            event_count,
        });
    }

    summaries.sort_by(|lhs, rhs| {
        lhs.spec
            .cmp(&rhs.spec)
            .then(lhs.profile.cmp(&rhs.profile))
            .then(lhs.engine.cmp(&rhs.engine))
    });
    Ok(summaries)
}

fn manifest_matches(manifest: &ManifestRecord, filter: &ManifestFilter) -> bool {
    filter
        .spec
        .as_ref()
        .is_none_or(|spec| manifest.spec == *spec)
        && filter.binding.as_ref().is_none_or(|binding| {
            let binding = normalize_binding_path(binding);
            manifest.binding == binding
                || manifest
                    .binding
                    .rsplit("::")
                    .next()
                    .is_some_and(|tail| tail == binding)
        })
        && filter
            .profile
            .as_ref()
            .is_none_or(|profile| manifest.profile == *profile)
}

fn normalize_binding_path(raw: &str) -> String {
    raw.split("::")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("::")
}

fn dedupe_manifests(manifests: Vec<ManifestRecord>) -> Vec<ManifestRecord> {
    let mut unique = BTreeMap::new();
    for manifest in manifests {
        unique.insert(manifest_identity_key(&manifest), manifest);
    }
    unique.into_values().collect()
}

fn manifest_identity_key(manifest: &ManifestRecord) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        manifest.spec,
        manifest.spec_path,
        manifest.export_module,
        manifest.crate_manifest_dir,
        manifest.binding,
        manifest.profile,
    )
}

fn manifest_group_key(manifest: &ManifestRecord) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        manifest.spec,
        manifest.spec_path,
        manifest.export_module,
        manifest.crate_manifest_dir,
        manifest.binding,
    )
}

fn augment_default_profiles(
    base: &Path,
    manifests: Vec<ManifestRecord>,
    filter: &ManifestFilter,
) -> Result<Vec<ManifestRecord>> {
    let mut grouped = BTreeMap::<String, Vec<ManifestRecord>>::new();
    for manifest in manifests {
        grouped
            .entry(manifest_group_key(&manifest))
            .or_default()
            .push(manifest);
    }

    let mut expanded = Vec::new();
    for mut group in grouped.into_values() {
        let Some(seed_manifest) = group.first().cloned() else {
            continue;
        };
        let present_profiles = group
            .iter()
            .map(|manifest| manifest.profile.clone())
            .collect::<BTreeSet<_>>();
        let missing_profiles = seed_manifest
            .default_profiles
            .iter()
            .filter(|profile| !present_profiles.contains(*profile))
            .filter(|profile| {
                filter
                    .profile
                    .as_ref()
                    .is_none_or(|wanted| wanted == *profile)
            })
            .cloned()
            .collect::<Vec<_>>();

        if !missing_profiles.is_empty() {
            let synthesized =
                synthesize_profiles_for_manifest(base, &seed_manifest, &missing_profiles)?;
            for profile in synthesized {
                group.push(ManifestRecord {
                    profile: profile.profile,
                    engine: profile.engine,
                    coverage: profile.coverage,
                    ..seed_manifest.clone()
                });
            }
        }

        expanded.extend(group);
    }

    Ok(dedupe_manifests(expanded))
}

fn synthesize_profiles_for_manifest(
    base: &Path,
    manifest: &ManifestRecord,
    labels: &[String],
) -> Result<Vec<ProfileRecord>> {
    let base = absolute_path(base)?;
    let helper_dir = helper_dir_for_manifest(&base, manifest).join("profiles");
    let src_dir = helper_dir.join("src");
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("failed to create {}", src_dir.display()))?;
    let workspace_root = workspace_root_from_base(&base);
    fs::write(
        helper_dir.join("Cargo.toml"),
        render_helper_cargo_toml(manifest, &workspace_root),
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            helper_dir.join("Cargo.toml").display()
        )
    })?;
    fs::write(
        src_dir.join("main.rs"),
        render_profile_helper_main_rs(manifest, labels),
    )
    .with_context(|| format!("failed to write {}", src_dir.join("main.rs").display()))?;

    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(helper_dir.join("Cargo.toml"))
        .current_dir(&workspace_root)
        .env(
            "CARGO_TARGET_DIR",
            workspace_root
                .join("target")
                .join("nirvash")
                .join("materialize-helper-target"),
        )
        .output()
        .with_context(|| "failed to run profile synthesis helper".to_owned())?;
    if !output.status.success() {
        bail!(
            "profile synthesis helper failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut profiles = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        profiles.push(
            serde_json::from_str::<ProfileRecord>(line)
                .with_context(|| format!("failed to decode helper profile line `{line}`"))?,
        );
    }
    Ok(profiles)
}

fn latest_matching_replay_bundle(base: &Path, manifest: &ManifestRecord) -> Result<PathBuf> {
    let replay_dir = base.join("replay");
    if !replay_dir.exists() {
        bail!(
            "no replay bundle found for spec={} profile={}",
            manifest.spec,
            manifest.profile
        );
    }

    let mut matches = Vec::new();
    for path in read_json_files(&replay_dir)? {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !file_name.ends_with("_bundle.json") {
            continue;
        }
        let bundle = read_replay_record(&path)?;
        if bundle.spec == manifest.spec && bundle.profile == manifest.profile {
            matches.push(path);
        }
    }
    matches.sort();
    matches.pop().with_context(|| {
        format!(
            "no replay bundle found for spec={} profile={}",
            manifest.spec, manifest.profile
        )
    })
}

fn read_json_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to enumerate {}", dir.display()))?;
    paths.retain(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"));
    paths.sort();
    Ok(paths)
}

fn read_replay_record(path: &Path) -> Result<ReplayBundleRecord> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to decode replay bundle {}", path.display()))
}

fn canonical_bundle_json_path(path: &Path) -> Result<PathBuf> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        return Ok(path.to_path_buf());
    }
    if path.extension().and_then(|ext| ext.to_str()) == Some("ndjson") {
        let json_path = path.with_extension("json");
        if json_path.exists() {
            return Ok(json_path);
        }
    }
    bail!(
        "replay path must point to a replay bundle .json or sibling .ndjson: {}",
        path.display()
    )
}

fn workspace_root_from_base(base: &Path) -> PathBuf {
    let components = base
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect::<Vec<_>>();
    if let Some(target_index) = components
        .iter()
        .rposition(|component| component == "target")
    {
        if target_index > 0 {
            let mut root = PathBuf::new();
            for component in &components[..target_index] {
                root.push(component);
            }
            if !root.as_os_str().is_empty() {
                return root;
            }
        }
    }

    base.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| base.to_path_buf())
}

fn materialized_output_dir(base: &Path, manifest: &ManifestRecord) -> PathBuf {
    if !manifest.crate_manifest_dir.is_empty() {
        return PathBuf::from(&manifest.crate_manifest_dir)
            .join("tests")
            .join("generated");
    }
    workspace_root_from_base(base)
        .join("tests")
        .join("generated")
}

fn materialized_index_path(base: &Path, manifest: &ManifestRecord) -> PathBuf {
    if !manifest.crate_manifest_dir.is_empty() {
        return PathBuf::from(&manifest.crate_manifest_dir)
            .join("tests")
            .join("generated.rs");
    }
    workspace_root_from_base(base)
        .join("tests")
        .join("generated.rs")
}

fn refresh_materialized_index(materialized_dir: &Path, index_path: &Path) -> Result<()> {
    let mut generated_files = fs::read_dir(materialized_dir)
        .with_context(|| format!("failed to read {}", materialized_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to enumerate {}", materialized_dir.display()))?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect::<Vec<_>>();
    generated_files.sort();

    let mut body = String::from(
        "#![allow(non_snake_case)]\n\
         // Generated by `cargo nirvash materialize-tests`\n\
         // Cargo discovers this integration test crate and includes files from tests/generated/*.rs.\n\n",
    );
    for path in generated_files {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("invalid generated replay filename: {}", path.display()))?;
        let module_name = sanitize_ident(
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    anyhow!("invalid generated replay module name: {}", path.display())
                })?,
        );
        body.push_str(&format!(
            "#[path = \"generated/{file_name}\"]\nmod {module_name};\n"
        ));
    }
    fs::write(index_path, body)
        .with_context(|| format!("failed to write {}", index_path.display()))?;
    Ok(())
}

fn render_materialized_test(manifest: &ManifestRecord, replay_path: &Path) -> String {
    let crate_ident = crate_root_ident(&manifest.crate_package);
    let export_module = normalize_subject_path(&manifest.export_module, &crate_ident);
    let binding = normalize_subject_path(&manifest.binding, &crate_ident);
    let test_name = format!(
        "replay_{}_{}",
        sanitize_ident(manifest_spec_stem(manifest)),
        sanitize_ident(&manifest.profile),
    );
    format!(
        "// Generated by `cargo nirvash materialize-tests`\n\
         // run with `cargo test -- --nocapture` to see tracing::debug route logs\n\
         // spec: {spec}\n\
         // binding: {binding}\n\
         // replay: {replay}\n\n\
         #[test]\n\
         fn {test_name}() {{\n\
             let _guard = ::nirvash_conformance::__enter_generated_test_tracing();\n\
             ::nirvash_conformance::__debug_materialized_replay_test_start(\n\
                 \"{test_name}\",\n\
                 \"{spec}\",\n\
                 \"{profile}\",\n\
                 \"{binding}\",\n\
                 ::std::path::Path::new(r#\"{replay}\"#),\n\
             );\n\
             {export_module}::replay::run::<{binding}>(r#\"{replay}\"#)\n\
                 .expect(\"materialized replay should pass\");\n\
         }}\n",
        spec = manifest.spec,
        profile = manifest.profile,
        binding = binding,
        export_module = export_module,
        replay = replay_path.display(),
        test_name = test_name,
    )
}

fn helper_dir_for_manifest(base: &Path, manifest: &ManifestRecord) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    manifest.spec.hash(&mut hasher);
    manifest.spec_slug.hash(&mut hasher);
    manifest.binding.hash(&mut hasher);
    manifest.profile.hash(&mut hasher);
    manifest.export_module.hash(&mut hasher);
    manifest.spec_path.hash(&mut hasher);
    manifest.crate_package.hash(&mut hasher);
    manifest.crate_manifest_dir.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());
    base.join("materialize-helper").join(hash)
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .with_context(|| "failed to resolve current directory".to_owned())?
            .join(path))
    }
}

fn crate_root_ident(package: &str) -> String {
    if package.is_empty() {
        "crate".to_owned()
    } else {
        package.replace('-', "_")
    }
}

fn manifest_spec_stem(manifest: &ManifestRecord) -> &str {
    if manifest.spec_slug.is_empty() {
        &manifest.spec
    } else {
        &manifest.spec_slug
    }
}

fn normalize_subject_path(path: &str, root: &str) -> String {
    let mut segments = path.split("::");
    let Some(head) = segments.next() else {
        return root.to_owned();
    };
    let tail = segments.collect::<Vec<_>>();
    if head == "crate" {
        if tail.is_empty() {
            root.to_owned()
        } else {
            format!("{root}::{}", tail.join("::"))
        }
    } else {
        path.to_owned()
    }
}

fn normalize_external_module_path(path: &str) -> String {
    let mut segments = path.split("::");
    let _ = segments.next();
    let tail = segments.collect::<Vec<_>>();
    if tail.is_empty() {
        "subject_crate".to_owned()
    } else {
        format!("subject_crate::{}", tail.join("::"))
    }
}

fn local_conformance_crate_dir(workspace_root: &Path) -> Option<PathBuf> {
    local_workspace_crate_dir(workspace_root, "nirvash-conformance")
}

fn local_workspace_crate_dir(workspace_root: &Path, crate_name: &str) -> Option<PathBuf> {
    let mut cursor = Some(workspace_root);
    while let Some(root) = cursor {
        let crate_dir = root.join("crates").join(crate_name);
        if crate_dir.join("Cargo.toml").is_file() {
            return Some(crate_dir);
        }
        cursor = root.parent();
    }
    None
}

fn render_helper_cargo_toml(manifest: &ManifestRecord, workspace_root: &Path) -> String {
    let conformance_dependency =
        if let Some(conformance_path) = local_conformance_crate_dir(workspace_root) {
            format!(
                "nirvash-conformance = {{ version = {version:?}, path = {path:?} }}",
                version = env!("CARGO_PKG_VERSION"),
                path = conformance_path.display().to_string(),
            )
        } else {
            format!(
                "nirvash-conformance = {{ version = {version:?} }}",
                version = env!("CARGO_PKG_VERSION"),
            )
        };
    format!(
        "[package]\nname = \"nirvash_materialize_helper\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nsubject_crate = {{ package = {crate_package:?}, path = {crate_manifest_dir:?} }}\n{conformance_dependency}\nserde_json = \"1\"\n",
        crate_package = manifest.crate_package,
        crate_manifest_dir = manifest.crate_manifest_dir,
        conformance_dependency = conformance_dependency,
    )
}

fn render_profile_helper_main_rs(manifest: &ManifestRecord, labels: &[String]) -> String {
    let export_module = normalize_external_module_path(&manifest.export_module);
    let encoded_labels = serde_json::to_string(labels).expect("encode profile labels");
    format!(
        "fn main() {{\n    let labels: ::std::vec::Vec<::std::string::String> = serde_json::from_str(r#\"{encoded_labels}\"#)\n        .expect(\"profile labels should decode\");\n    for label in labels {{\n        if let Some(profile) = {export_module}::plans::profile_for_label(&label) {{\n            println!(\"{{}}\", serde_json::json!({{\n                \"profile\": label,\n                \"engine\": profile.engines,\n                \"coverage\": profile.coverage,\n            }}));\n        }}\n    }}\n}}\n",
        encoded_labels = encoded_labels,
        export_module = export_module,
    )
}

fn validate_ndjson(path: &Path) -> Result<usize> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut count = 0;
    for (index, line) in content.lines().enumerate() {
        serde_json::from_str::<Value>(line).with_context(|| {
            format!(
                "failed to decode NDJSON line {} in {}",
                index + 1,
                path.display()
            )
        })?;
        count += 1;
    }
    Ok(count)
}

fn sanitize_path(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect()
}

fn sanitize_ident(raw: &str) -> String {
    let mut chars = raw.chars();
    let first = chars.next().unwrap_or('_');
    let mut ident = String::new();
    ident.push(match first {
        'a'..='z' | 'A'..='Z' | '_' => first,
        '0'..='9' => '_',
        _ => '_',
    });
    ident.extend(chars.map(|ch| match ch {
        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => ch,
        _ => '_',
    }));
    ident
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn manifest_record(crate_manifest_dir: &Path) -> ManifestRecord {
        ManifestRecord {
            spec: "demo.spec".to_owned(),
            spec_slug: String::new(),
            spec_path: "demo_crate::DemoSpec".to_owned(),
            export_module: "crate::demo::generated".to_owned(),
            crate_package: "demo-crate".to_owned(),
            crate_manifest_dir: crate_manifest_dir.display().to_string(),
            default_profiles: vec!["small".to_owned()],
            binding: "crate::demo::DemoBinding".to_owned(),
            profile: "small".to_owned(),
            engine: serde_json::json!(["explicit_suite"]),
            coverage: vec![serde_json::json!("transitions")],
            replay_dir: Some("target/nirvash/replay".to_owned()),
            materialize_failures: true,
        }
    }

    fn manifest_json_with_binding_aliases(crate_manifest_dir: &Path) -> serde_json::Value {
        serde_json::json!({
            "spec_name": "demo.spec",
            "spec_slug": "",
            "spec_path": "demo_crate::DemoSpec",
            "export_module": "crate::demo::generated",
            "crate_package": "demo-crate",
            "crate_manifest_dir": crate_manifest_dir.display().to_string(),
            "default_profiles": ["small"],
            "binding": "crate::demo::DemoBinding",
            "binding_path": "crate::demo::DemoBinding",
            "profile": "small",
            "engines": ["explicit_suite"],
            "coverage": ["transitions"],
            "replay_dir": "target/nirvash/replay",
            "materialize_failures": true
        })
    }

    fn write_replay_files(base: &Path) -> (PathBuf, PathBuf) {
        let replay_dir = base.join("replay");
        fs::create_dir_all(&replay_dir).expect("replay dir");
        let json_path = replay_dir.join("demo_spec_small_explicit_bundle.json");
        let ndjson_path = replay_dir.join("demo_spec_small_explicit_bundle.ndjson");
        fs::write(
            &json_path,
            serde_json::to_vec_pretty(&ReplayBundleRecord {
                spec: "demo.spec".to_owned(),
                profile: "small".to_owned(),
                engine: "explicit".to_owned(),
                detail: serde_json::json!({ "events": [] }),
                action_trace: serde_json::json!({ "steps": [] }),
            })
            .expect("encode replay"),
        )
        .expect("write replay");
        fs::write(
            &ndjson_path,
            [
                serde_json::json!({ "kind": "initial", "state": "idle" }).to_string(),
                serde_json::json!({ "kind": "stutter" }).to_string(),
            ]
            .join("\n"),
        )
        .expect("write ndjson");
        (json_path, ndjson_path)
    }

    #[test]
    fn list_tests_filters_by_spec_binding_and_profile() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let manifest_dir = base.join("manifest");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");
        let manifest_path = manifest_dir.join("demo_spec__demo_binding__small.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest_record(&crate_manifest_dir))
                .expect("encode manifest"),
        )
        .expect("write manifest");

        let manifests = list_tests(
            &base,
            &ManifestFilter {
                spec: Some("demo.spec".to_owned()),
                binding: Some("DemoBinding".to_owned()),
                profile: Some("small".to_owned()),
            },
        )
        .expect("list manifests");

        assert_eq!(manifests, vec![manifest_record(&crate_manifest_dir)]);
    }

    #[test]
    fn list_tests_decodes_manifest_with_binding_and_binding_path() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let manifest_dir = base.join("manifest");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");
        let manifest_path = manifest_dir.join("demo_spec__demo_binding__small.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest_json_with_binding_aliases(&crate_manifest_dir))
                .expect("encode manifest"),
        )
        .expect("write manifest");

        let manifests = list_tests(&base, &ManifestFilter::default()).expect("list manifests");

        assert_eq!(manifests, vec![manifest_record(&crate_manifest_dir)]);
    }

    #[test]
    fn list_tests_normalizes_binding_whitespace_and_dedupes_stale_manifests() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let manifest_dir = base.join("manifest");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");

        let first_path = manifest_dir.join("demo_spec__binding_spaced__small.json");
        let second_path = manifest_dir.join("demo_spec__binding_normalized__small.json");
        let mut first = manifest_json_with_binding_aliases(&crate_manifest_dir);
        first["binding"] = serde_json::json!("crate :: demo :: DemoBinding");
        first["binding_path"] = serde_json::json!("crate :: demo :: DemoBinding");
        let second = manifest_json_with_binding_aliases(&crate_manifest_dir);
        fs::write(
            &first_path,
            serde_json::to_vec_pretty(&first).expect("encode manifest"),
        )
        .expect("write spaced manifest");
        fs::write(
            &second_path,
            serde_json::to_vec_pretty(&second).expect("encode manifest"),
        )
        .expect("write normalized manifest");

        let manifests = list_tests(
            &base,
            &ManifestFilter {
                spec: Some("demo.spec".to_owned()),
                binding: Some("crate :: demo :: DemoBinding".to_owned()),
                profile: Some("small".to_owned()),
            },
        )
        .expect("list manifests");

        assert_eq!(manifests, vec![manifest_record(&crate_manifest_dir)]);
    }

    #[test]
    fn materialize_tests_writes_rust_replay_file() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let manifest_dir = base.join("manifest");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");
        let manifest_path = manifest_dir.join("demo_spec__demo_binding__small.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest_record(&crate_manifest_dir))
                .expect("encode manifest"),
        )
        .expect("write manifest");
        let (json_path, _) = write_replay_files(&base);

        let materialized = materialize_tests(
            &base,
            &MaterializeRequest {
                spec: "demo.spec".to_owned(),
                binding: "DemoBinding".to_owned(),
                profile: "small".to_owned(),
                replay: Some(json_path.clone()),
            },
        )
        .expect("materialize replay");

        assert_eq!(materialized.len(), 1);
        assert_eq!(
            materialized[0],
            crate_manifest_dir.join("tests/generated/demo_spec_small_replay.rs")
        );
        let body = fs::read_to_string(&materialized[0]).expect("read materialized file");
        assert!(
            body.contains(
                "demo_crate::demo::generated::replay::run::<demo_crate::demo::DemoBinding>"
            )
        );
        assert!(body.contains(&json_path.display().to_string()));
        let index = crate_manifest_dir.join("tests/generated.rs");
        let index_body = fs::read_to_string(&index).expect("read generated index");
        assert!(index_body.contains("#[path = \"generated/demo_spec_small_replay.rs\"]"));
        assert!(index_body.contains("mod demo_spec_small_replay;"));
    }

    #[test]
    fn materialize_tests_accepts_binding_whitespace_aliases() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let manifest_dir = base.join("manifest");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");
        let manifest_path = manifest_dir.join("demo_spec__demo_binding__small.json");
        let mut manifest = manifest_json_with_binding_aliases(&crate_manifest_dir);
        manifest["binding"] = serde_json::json!("crate :: demo :: DemoBinding");
        manifest["binding_path"] = serde_json::json!("crate :: demo :: DemoBinding");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
        )
        .expect("write manifest");
        let (json_path, _) = write_replay_files(&base);

        let materialized = materialize_tests(
            &base,
            &MaterializeRequest {
                spec: "demo.spec".to_owned(),
                binding: "crate :: demo :: DemoBinding".to_owned(),
                profile: "small".to_owned(),
                replay: Some(json_path),
            },
        )
        .expect("materialize replay");

        assert_eq!(materialized.len(), 1);
        assert_eq!(
            materialized[0],
            crate_manifest_dir.join("tests/generated/demo_spec_small_replay.rs")
        );
    }

    #[test]
    fn materialize_tests_prefers_spec_slug_for_output_paths() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let manifest_dir = base.join("manifest");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");
        let mut manifest = manifest_record(&crate_manifest_dir);
        manifest.spec_slug = "tests_fixtures_demo_spec_line42_col1_Spec".to_owned();
        let manifest_path = manifest_dir.join("demo_spec__demo_binding__small.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
        )
        .expect("write manifest");
        let (json_path, _) = write_replay_files(&base);

        let materialized = materialize_tests(
            &base,
            &MaterializeRequest {
                spec: "demo.spec".to_owned(),
                binding: "DemoBinding".to_owned(),
                profile: "small".to_owned(),
                replay: Some(json_path),
            },
        )
        .expect("materialize replay");

        assert_eq!(
            materialized[0],
            crate_manifest_dir
                .join("tests/generated/tests_fixtures_demo_spec_line42_col1_Spec_small_replay.rs")
        );
    }

    #[test]
    fn materialize_replay_uses_ndjson_path_as_sugar() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let manifest_dir = base.join("manifest");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");
        let manifest_path = manifest_dir.join("demo_spec__demo_binding__small.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest_record(&crate_manifest_dir))
                .expect("encode manifest"),
        )
        .expect("write manifest");
        let (_, ndjson_path) = write_replay_files(&base);

        let materialized = materialize_replay(&base, &ndjson_path).expect("materialize replay");

        assert_eq!(materialized.len(), 1);
        assert_eq!(
            materialized[0],
            crate_manifest_dir.join("tests/generated/demo_spec_small_replay.rs")
        );
    }

    #[test]
    fn replay_reads_json_and_ndjson_pairs() {
        let temp = tempdir().expect("tempdir");
        let base = temp.path().join("nirvash");
        let (_, ndjson_path) = write_replay_files(&base);

        let summaries = replay(&base).expect("replay summary");

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].spec, "demo.spec");
        assert_eq!(summaries[0].event_count, 2);
        assert_eq!(summaries[0].ndjson_path, ndjson_path);
    }

    #[test]
    fn helper_manifest_uses_registry_dependency_outside_nirvash_workspace() {
        let temp = tempdir().expect("tempdir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");

        let body = render_helper_cargo_toml(&manifest_record(&crate_manifest_dir), temp.path());
        let conformance_line = body
            .lines()
            .find(|line| line.starts_with("nirvash-conformance = "))
            .expect("conformance dependency line");

        assert_eq!(
            conformance_line,
            "nirvash-conformance = { version = \"0.1.0\" }"
        );
    }

    #[test]
    fn helper_manifest_prefers_local_workspace_dependency_when_available() {
        let temp = tempdir().expect("tempdir");
        let crate_manifest_dir = temp.path().join("demo-crate");
        let conformance_dir = temp.path().join("crates").join("nirvash-conformance");
        fs::create_dir_all(&crate_manifest_dir).expect("crate manifest dir");
        fs::create_dir_all(&conformance_dir).expect("conformance dir");
        fs::write(
            conformance_dir.join("Cargo.toml"),
            "[package]\nname = \"nirvash-conformance\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("write conformance manifest");

        let body = render_helper_cargo_toml(&manifest_record(&crate_manifest_dir), temp.path());

        assert!(body.contains("nirvash-conformance = { version = \"0.1.0\", path = "));
        assert!(body.contains(&conformance_dir.display().to_string()));
    }
}
