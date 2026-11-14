use super::super::builtin::Context;
use rayon::prelude::*;
use std::sync::OnceLock;

/// Resolve dynamic eligibility only for menu providers that passed their
/// static match and basic capability filters. The menu waits for these results
/// so its first-level contents are stable when displayed.
pub(in crate::plugins::providers::context_menu) fn preflight_availability(
    context: &Context,
) -> super::super::builtin::MenuPreflightAvailability {
    let registry = super::registry();
    let matched = registry
        .matched_candidates(
            &context.match_targets,
            context.directory_target.as_ref(),
            context.paths.len(),
        )
        .into_iter()
        .filter(|candidate| {
            registry.module_id(&candidate.id).as_deref() == Some(fiex_actions::MODULE_ID)
        })
        .map(|candidate| candidate.id)
        .collect::<std::collections::HashSet<_>>();
    let capabilities = context.capabilities();
    let mut result = super::super::builtin::MenuPreflightAvailability::default();

    if context.paths.is_empty() {
        let check_file = capabilities.write && matched.contains("builtin:new-file");
        let check_folder = capabilities.create && matched.contains("builtin:new-folder");
        if (check_file || check_folder)
            && let Ok(parent) = crate::core::fs::trusted(&context.directory, "core:context-menu")
        {
            result.new_file = check_file && parent.review_create_available(false);
            result.new_folder = check_folder && parent.review_create_available(true);
        }
        return result;
    }

    let check_rename = capabilities.rename
        && ((context.paths.len() == 1 && matched.contains("builtin:rename"))
            || (context.paths.len() > 1 && matched.contains("builtin:batch-rename")));
    let check_trash = capabilities.trash && matched.contains("builtin:move-to-trash");
    let check_remove = capabilities.remove && matched.contains("builtin:delete-permanently");
    if !(check_rename || check_trash || check_remove) {
        return result;
    }

    let availability = preflight_for_paths(&context.paths, check_rename, check_trash, check_remove);
    let all_checked = !availability.is_empty();
    if context.paths.len() == 1 {
        result.rename =
            check_rename && all_checked && availability.iter().all(|value| value.rename);
    } else {
        result.batch_rename =
            check_rename && all_checked && availability.iter().all(|value| value.rename);
    }
    result.trash = check_trash && all_checked && availability.iter().all(|value| value.trash);
    result.remove = check_remove && all_checked && availability.iter().all(|value| value.remove);
    result
}

#[derive(Default)]
struct ResourceAvailability {
    rename: bool,
    trash: bool,
    remove: bool,
}

/// Reuse one trusted resource and run all applicable checks on one bounded
/// worker lane per selected path.
fn preflight_for_paths(
    paths: &[String],
    rename: bool,
    trash: bool,
    remove: bool,
) -> Vec<ResourceAvailability> {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    let pool = POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .thread_name(|index| format!("menu-preflight-{index}"))
            .build()
            .expect("menu preflight pool configuration is valid")
    });
    let unique = paths.iter().collect::<std::collections::HashSet<_>>();
    pool.install(|| {
        unique
            .par_iter()
            .map(|path| {
                let Ok(resource) = crate::core::fs::trusted(path, "core:context-menu") else {
                    return ResourceAvailability::default();
                };
                ResourceAvailability {
                    rename: rename && resource.review_rename_available(),
                    trash: trash && resource.review_trash_available(),
                    remove: remove && resource.review_remove_available(),
                }
            })
            .collect::<Vec<_>>()
    })
}
