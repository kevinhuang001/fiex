//! Pure permission policy for plugin filesystem and view scopes.

mod derived;
pub use derived::DerivedReferenceGrants;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodDenial {
    UnsupportedFilesystemNamespace,
    ReadOnly,
    MetadataOnly,
}

const READ_ONLY_DENIED_METHODS: &[&str] = &[
    "host.fs.mkdir",
    "host.fs.seekWrite",
    "host.fs.beginWrite",
    "host.fs.writeChunk",
    "host.fs.writeBuffer",
    "host.fs.commit",
    "host.fs.remove",
    "host.fs.trash",
    "host.fs.rename",
    "host.process.run",
    "host.vfs.openView",
];

#[cfg(any(test, kani))]
const METADATA_ONLY_DENIED_METHODS: &[&str] = &[
    "host.fs.mkdir",
    "host.fs.seekWrite",
    "host.fs.beginWrite",
    "host.fs.writeChunk",
    "host.fs.writeBuffer",
    "host.fs.commit",
    "host.fs.remove",
    "host.fs.trash",
    "host.fs.rename",
    "host.fs.openRead",
    "host.fs.readChunk",
    "host.fs.write",
    "host.fs.search",
    "host.fs.list",
    "host.fs.readSmall",
    "host.fs.readText",
    "host.process.run",
    "host.vfs.openView",
    "host.fs.futureMethod",
];

/// Returns the first scope restriction that denies a host method.
///
/// The order is part of the public error behavior: the legacy `fs.*`
/// namespace is rejected before per-scope restrictions are considered.
pub fn method_denial(method: &str, metadata_only: bool, read_only: bool) -> Option<MethodDenial> {
    if method.starts_with("fs.") {
        return Some(MethodDenial::UnsupportedFilesystemNamespace);
    }
    if read_only && READ_ONLY_DENIED_METHODS.contains(&method) {
        return Some(MethodDenial::ReadOnly);
    }
    if metadata_only
        && ((method.starts_with("host.fs.")
            && !matches!(method, "host.fs.stat" | "host.fs.statBatch"))
            || matches!(method, "host.process.run" | "host.vfs.openView"))
    {
        return Some(MethodDenial::MetadataOnly);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        METADATA_ONLY_DENIED_METHODS, MethodDenial, READ_ONLY_DENIED_METHODS, method_denial,
    };

    #[test]
    fn read_only_scopes_reject_every_mutating_or_process_method() {
        for method in READ_ONLY_DENIED_METHODS {
            assert_eq!(
                method_denial(method, false, true),
                Some(MethodDenial::ReadOnly),
                "{method}"
            );
        }
    }

    #[test]
    fn metadata_scopes_reject_every_non_metadata_method() {
        for method in METADATA_ONLY_DENIED_METHODS {
            assert_eq!(
                method_denial(method, true, false),
                Some(MethodDenial::MetadataOnly),
                "{method}"
            );
        }
        assert_eq!(method_denial("host.fs.stat", true, false), None);
        assert_eq!(method_denial("host.fs.statBatch", true, false), None);
        assert_eq!(
            method_denial("host.fs.futureMethod", true, false),
            Some(MethodDenial::MetadataOnly)
        );
    }

    #[test]
    fn legacy_filesystem_namespace_is_rejected_for_every_scope() {
        for metadata_only in [false, true] {
            for read_only in [false, true] {
                assert_eq!(
                    method_denial("fs.read", metadata_only, read_only),
                    Some(MethodDenial::UnsupportedFilesystemNamespace)
                );
            }
        }
    }

    #[test]
    fn read_only_denial_takes_precedence_for_metadata_scopes() {
        assert_eq!(
            method_denial("host.fs.writeChunk", true, true),
            Some(MethodDenial::ReadOnly)
        );
    }
}

#[cfg(kani)]
mod verification {
    use super::{
        METADATA_ONLY_DENIED_METHODS, MethodDenial, READ_ONLY_DENIED_METHODS, method_denial,
    };

    #[kani::proof]
    fn read_only_scopes_deny_all_declared_mutation_methods() {
        let metadata_only: bool = kani::any();
        for method in READ_ONLY_DENIED_METHODS {
            assert_eq!(
                method_denial(method, metadata_only, true),
                Some(MethodDenial::ReadOnly)
            );
        }
    }

    #[kani::proof]
    fn metadata_only_scopes_deny_all_declared_non_metadata_methods() {
        let read_only: bool = kani::any();
        for method in METADATA_ONLY_DENIED_METHODS {
            let expected = if read_only && READ_ONLY_DENIED_METHODS.contains(method) {
                MethodDenial::ReadOnly
            } else {
                MethodDenial::MetadataOnly
            };
            assert_eq!(method_denial(method, true, read_only), Some(expected));
        }
    }

    #[kani::proof]
    fn legacy_namespace_is_denied_independent_of_scope_flags() {
        let metadata_only: bool = kani::any();
        let read_only: bool = kani::any();
        assert_eq!(
            method_denial("fs.read", metadata_only, read_only),
            Some(MethodDenial::UnsupportedFilesystemNamespace)
        );
    }
}
