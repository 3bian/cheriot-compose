// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use crate::configuration::{
    CompilerFlag, ComponentRecipe, ComponentRole, ResolvedProject, RevokerKind, SdkComponentKind,
    TargetKind,
};
use crate::diagnostic::{Diagnostic, Origin, Result};

mod core_flags;
mod standard_recipes;

pub const SDK_REVISION: &str = "d0684c41a379560c5d9065539795e2664c8e6f27";

pub fn resolve_dependencies(project: &ResolvedProject) -> Result<Vec<ComponentRecipe>> {
    let mut recipes = standard_recipes::standard_recipes(project);
    let scheduler = format!("{}.scheduler", project.firmware.name);
    for (name, target) in &project.targets {
        if recipes.contains_key(name) && !project.sdk_component_names.contains(name) {
            return Err(Diagnostic::new(format!(
                "target {name} conflicts with an SDK target"
            )));
        }
        if let Some(existing) = recipes.get(name)
            && project.sdk_component_names.contains(name)
            && (existing.uses_bare_metal_abi()
                || existing.needs_subobject_bounds_probe()
                || matches!(
                    existing.kind,
                    SdkComponentKind::PrivilegedCompartment
                        | SdkComponentKind::PrivilegedLibrary
                        | SdkComponentKind::Loader
                ))
        {
            return Err(Diagnostic::new(format!(
                "SDK component {name} requires built-in special rules and cannot be replaced"
            )));
        }
        if let Some(existing) = recipes.get(name)
            && project.sdk_component_names.contains(name)
        {
            let expected = match target.kind {
                TargetKind::Compartment => SdkComponentKind::Compartment,
                TargetKind::Library => SdkComponentKind::Library,
                TargetKind::Object => SdkComponentKind::Object,
                TargetKind::Group => SdkComponentKind::Group,
            };
            if existing.kind != expected {
                return Err(Diagnostic::new(format!(
                    "SDK component {name} has a different kind from the built-in recipe"
                )));
            }
        }
        recipes.insert(
            name.clone(),
            ComponentRecipe {
                name: name.clone(),
                kind: match target.kind {
                    TargetKind::Compartment => SdkComponentKind::Compartment,
                    TargetKind::Library => SdkComponentKind::Library,
                    TargetKind::Object => SdkComponentKind::Object,
                    TargetKind::Group => SdkComponentKind::Group,
                },
                role: target.role,
                sources: target.sources.clone(),
                compiler: target.compiler.clone(),
                dependencies: target.dependencies.clone(),
                flags: target.flags.clone(),
                c_flags: target.c_flags.clone(),
                cpp_flags: target.cpp_flags.clone(),
                assembly_flags: target.assembly_flags.clone(),
                rust_flags: target.rust_flags.clone(),
                cargo_flags: target.cargo_flags.clone(),
                includes: target.includes.clone(),
                force_includes: target.force_includes.clone(),
                compartment: (target.kind == TargetKind::Compartment).then(|| name.clone()),
                script: if target.kind == TargetKind::Library {
                    "library.ldscript"
                } else {
                    "compartment.ldscript"
                }
                .into(),
            },
        );
    }
    for recipe in recipes.values_mut() {
        if let Some(flags) = core_flags::for_role(project, recipe.role) {
            recipe.flags = flags;
            if recipe.role == Some(ComponentRole::SoftwareRevoker) {
                recipe.script = "software_revoker.ldscript".into();
            }
            continue;
        }
        if matches!(
            recipe.kind,
            SdkComponentKind::Library | SdkComponentKind::PrivilegedLibrary
        ) {
            let flag = CompilerFlag {
                value: "-DCHERIOT_NO_AMBIENT_MALLOC".into(),
                origin: Origin::derived("cheriot.library"),
            };
            if recipe.role == Some(ComponentRole::Softfloat) {
                recipe.flags.push(flag);
            } else {
                recipe.flags.insert(0, flag);
            }
        }
        let role = match recipe.role {
            Some(ComponentRole::Allocator) => "allocator",
            Some(ComponentRole::Loader) => "loader",
            Some(ComponentRole::Scheduler) => "scheduler",
            _ => "",
        };
        if let Some(defs) = project.board.rtos_defines.get(role) {
            for (definition_index, definition) in defs.iter().enumerate() {
                recipe.flags.push(CompilerFlag {
                    value: format!("-D{definition}"),
                    origin: project
                        .provenance
                        .get(&format!("/board/rtos_defines/{role}/{definition_index}"))
                        .cloned()
                        .unwrap_or_else(|| Origin::derived("board rtos_defines")),
                });
            }
        }
    }
    let mut roots = project.firmware.services.clone();
    roots.extend(project.firmware.libraries.clone());
    roots.extend([
        scheduler,
        "cheriot.loader".into(),
        "cheriot.switcher".into(),
        "cheriot.token_library".into(),
    ]);
    if project.options.allocator {
        roots.push("cheriot.allocator".into())
    }
    if project.board.revoker == RevokerKind::Software {
        roots.push("cheriot.software_revoker".into())
    }
    roots.sort();
    let mut ordered_recipes = Vec::new();
    let mut visited = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    fn visit_dependency(
        name: &str,
        recipes: &BTreeMap<String, ComponentRecipe>,
        visited: &mut BTreeSet<String>,
        visiting: &mut BTreeSet<String>,
        ordered_recipes: &mut Vec<ComponentRecipe>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Ok(());
        }
        if !visiting.insert(name.into()) {
            return Err(Diagnostic::new(format!(
                "target dependency cycle at {name}"
            )));
        }
        let recipe = recipes.get(name).ok_or_else(|| {
            Diagnostic::new(format!(
                "unknown or unsupported SDK target {name}; see the RTOS build coverage documentation"
            ))
        })?;
        let mut dependencies = recipe.dependencies.clone();
        dependencies.sort();
        for dependency in dependencies {
            visit_dependency(&dependency, recipes, visited, visiting, ordered_recipes)?
        }
        visiting.remove(name);
        visited.insert(name.into());
        ordered_recipes.push(recipe.clone());
        Ok(())
    }
    for name in roots {
        visit_dependency(
            &name,
            &recipes,
            &mut visited,
            &mut visiting,
            &mut ordered_recipes,
        )?
    }
    Ok(ordered_recipes)
}

pub fn link_input_order(recipes: &[ComponentRecipe]) -> Vec<&ComponentRecipe> {
    let recipes_by_name: BTreeMap<&str, &ComponentRecipe> = recipes
        .iter()
        .map(|recipe| (recipe.name.as_str(), recipe))
        .collect();
    fn visit_link_input<'a>(
        recipe: &'a ComponentRecipe,
        recipes_by_name: &BTreeMap<&str, &'a ComponentRecipe>,
        visited: &mut BTreeSet<String>,
        ordered_recipes: &mut Vec<&'a ComponentRecipe>,
    ) {
        if !visited.insert(recipe.name.clone()) {
            return;
        }
        ordered_recipes.push(recipe);
        let mut dependencies = recipe.dependencies.clone();
        dependencies.sort();
        for dependency in dependencies {
            if let Some(dependency_recipe) = recipes_by_name.get(dependency.as_str()) {
                visit_link_input(dependency_recipe, recipes_by_name, visited, ordered_recipes)
            }
        }
    }
    let mut visited = BTreeSet::new();
    let mut ordered_recipes = Vec::new();
    for recipe in recipes_by_name.values() {
        visit_link_input(recipe, &recipes_by_name, &mut visited, &mut ordered_recipes)
    }
    ordered_recipes
}
