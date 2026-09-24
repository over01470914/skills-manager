use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    pub schema_version: u32,
    pub slug: String,
    pub description: String,
    #[serde(default)]
    pub instructions: String,
    pub stages: Vec<BundleStage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleStage {
    pub id: String,
    pub skills: Vec<String>,
    #[serde(default)]
    pub after: Vec<String>,
    pub when: Option<String>,
}

pub fn read_manifest(path: &Path) -> Result<BundleManifest> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("cannot read bundle manifest {}", path.display()))?;
    let manifest: BundleManifest = serde_yaml::from_str(&content)
        .with_context(|| format!("invalid bundle YAML {}", path.display()))?;
    validate_structure(&manifest)?;
    Ok(manifest)
}

pub fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

pub fn validate_structure(manifest: &BundleManifest) -> Result<()> {
    if manifest.schema_version != 1 {
        bail!(
            "unsupported bundle schema_version: {}",
            manifest.schema_version
        );
    }
    if !valid_slug(&manifest.slug) {
        bail!("bundle slug must use lowercase letters, digits, and single hyphens");
    }
    if manifest.description.trim().is_empty() {
        bail!("bundle description is required");
    }
    if manifest.stages.is_empty() {
        bail!("bundle must contain at least one stage");
    }

    let mut ids = HashSet::new();
    for stage in &manifest.stages {
        if !valid_slug(&stage.id) || !ids.insert(stage.id.as_str()) {
            bail!("invalid or duplicate bundle stage id: {}", stage.id);
        }
        if stage.skills.is_empty() || stage.skills.iter().any(|skill| skill.trim().is_empty()) {
            bail!("stage {} must reference at least one named skill", stage.id);
        }
        if stage.skills.iter().collect::<HashSet<_>>().len() != stage.skills.len() {
            bail!(
                "stage {} references the same skill more than once",
                stage.id
            );
        }
        if stage.after.iter().collect::<HashSet<_>>().len() != stage.after.len() {
            bail!("stage {} repeats a predecessor", stage.id);
        }
        if let Some(fact) = &stage.when {
            if !valid_slug(fact) {
                bail!("invalid condition fact in stage {}: {}", stage.id, fact);
            }
        }
    }

    let stages: HashMap<_, _> = manifest.stages.iter().map(|s| (s.id.as_str(), s)).collect();
    for stage in &manifest.stages {
        for dependency in &stage.after {
            if dependency == &stage.id || !stages.contains_key(dependency.as_str()) {
                bail!("stage {} has invalid predecessor {}", stage.id, dependency);
            }
        }
    }

    fn visit<'a>(
        id: &'a str,
        stages: &HashMap<&'a str, &'a BundleStage>,
        visiting: &mut HashSet<&'a str>,
        visited: &mut HashSet<&'a str>,
    ) -> Result<()> {
        if visited.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id) {
            bail!("bundle stages contain a cycle at {id}");
        }
        for predecessor in &stages[id].after {
            visit(predecessor, stages, visiting, visited)?;
        }
        visiting.remove(id);
        visited.insert(id);
        Ok(())
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for stage in &manifest.stages {
        visit(&stage.id, &stages, &mut visiting, &mut visited)?;
    }
    Ok(())
}

pub fn ready_stage<'a>(
    manifest: &'a BundleManifest,
    stage_id: &str,
    completed: &HashSet<String>,
    facts: &HashMap<String, bool>,
) -> Result<&'a BundleStage> {
    let stage = manifest
        .stages
        .iter()
        .find(|stage| stage.id == stage_id)
        .with_context(|| format!("unknown bundle stage: {stage_id}"))?;
    for predecessor in &stage.after {
        if !completed.contains(predecessor) {
            bail!(
                "stage {} requires completed stage {}",
                stage.id,
                predecessor
            );
        }
    }
    if let Some(fact) = &stage.when {
        if facts.get(fact) != Some(&true) {
            bail!("stage {} requires fact {}=true", stage.id, fact);
        }
    }
    Ok(stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BundleManifest {
        serde_yaml::from_str(
            "schema_version: 1\nslug: web-release\ndescription: Web release\nstages:\n  - id: build\n    skills: [frontend-design]\n  - id: review\n    after: [build]\n    when: page-available\n    skills: [web-design-reviewer]\n",
        )
        .unwrap()
    }

    #[test]
    fn only_ready_stage_resolves() {
        let manifest = sample();
        validate_structure(&manifest).unwrap();
        assert!(ready_stage(&manifest, "review", &HashSet::new(), &HashMap::new()).is_err());
        let completed = HashSet::from(["build".to_string()]);
        assert!(ready_stage(&manifest, "review", &completed, &HashMap::new()).is_err());
        let facts = HashMap::from([("page-available".to_string(), true)]);
        assert_eq!(
            ready_stage(&manifest, "review", &completed, &facts)
                .unwrap()
                .skills
                .len(),
            1
        );
    }

    #[test]
    fn rejects_cycle() {
        let mut manifest = sample();
        manifest.stages[0].after.push("review".to_string());
        assert!(validate_structure(&manifest).is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let result: Result<BundleManifest, _> = serde_yaml::from_str(
            "schema_version: 1\nslug: test\ndescription: Test\nunknown: true\nstages: []\n",
        );
        assert!(result.is_err());
    }
}
