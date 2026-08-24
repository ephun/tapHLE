// Allow the crate to have a non-snake-case name (tapHLE).
// This also allows items in the crate to have non-snake-case names.
#![allow(non_snake_case)]

mod version_logic;

/// Current version. See `build.rs` for how this is generated.
pub const VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/version.txt"));

// Environment variables set by GitHub Actions
pub const GITHUB_REPOSITORY: Option<&str> = option_env!("GITHUB_REPOSITORY");
pub const GITHUB_SERVER_URL: Option<&str> = option_env!("GITHUB_SERVER_URL");
pub const GITHUB_RUN_ID: Option<&str> = option_env!("GITHUB_RUN_ID");
pub const GITHUB_REF_NAME: Option<&str> = option_env!("GITHUB_REF_NAME");
pub const GITHUB_REF_TYPE: Option<&str> = option_env!("GITHUB_REF_TYPE");
pub const GITHUB_EVENT_NAME: Option<&str> = option_env!("GITHUB_EVENT_NAME");

pub fn branding() -> &'static str {
    branding_for(
        GITHUB_RUN_ID.is_some(),
        VERSION,
        env!("CARGO_PKG_VERSION"),
        GITHUB_REPOSITORY,
        GITHUB_REF_NAME,
        GITHUB_REF_TYPE,
        GITHUB_EVENT_NAME,
    )
}

fn branding_for(
    is_github_build: bool,
    version: &str,
    cargo_version: &str,
    repository: Option<&str>,
    ref_name: Option<&str>,
    ref_type: Option<&str>,
    event_name: Option<&str>,
) -> &'static str {
    if !is_github_build {
        return "";
    }
    if event_name == Some("push")
        && repository == Some("ephun/tapHLE")
        && ref_type == Some("tag")
        && version == version_logic::display_release_version(cargo_version)
        && ref_name.is_some_and(|name| name == version_logic::release_tag(cargo_version))
    {
        return "";
    }
    if (repository, ref_name) == (Some("ephun/tapHLE"), Some("trunk")) {
        "PREVIEW"
    } else {
        "UNOFFICIAL"
    }
}

#[cfg(test)]
mod tests {
    use super::branding_for;

    #[test]
    fn only_exact_official_fork_tag_is_unbranded() {
        let args = (
            true,
            "v0.3.0-alpha.1",
            "0.3.0-alpha.1",
            Some("ephun/tapHLE"),
            Some("taphle-v0.3.0-alpha.1"),
            Some("tag"),
            Some("push"),
        );
        assert_eq!(
            branding_for(args.0, args.1, args.2, args.3, args.4, args.5, args.6),
            ""
        );

        assert_eq!(
            branding_for(
                true,
                "v0.3.0-alpha.1",
                "0.3.0-alpha.1",
                Some("ephun/tapHLE"),
                Some("v0.3.0-alpha.1"),
                Some("tag"),
                Some("push"),
            ),
            "UNOFFICIAL"
        );
        assert_eq!(
            branding_for(
                true,
                "v0.3.0-alpha.1-dirty",
                "0.3.0-alpha.1",
                Some("ephun/tapHLE"),
                Some("taphle-v0.3.0-alpha.1"),
                Some("tag"),
                Some("push"),
            ),
            "UNOFFICIAL"
        );
    }

    #[test]
    fn trunk_and_other_builds_keep_channel_branding() {
        assert_eq!(
            branding_for(
                true,
                "abc1234",
                "0.3.0-alpha.1",
                Some("ephun/tapHLE"),
                Some("trunk"),
                Some("branch"),
                Some("push"),
            ),
            "PREVIEW"
        );
        assert_eq!(
            branding_for(
                true,
                "abc1234",
                "0.3.0-alpha.1",
                Some("someone/fork"),
                Some("branch"),
                Some("branch"),
                Some("push"),
            ),
            "UNOFFICIAL"
        );
        assert_eq!(
            branding_for(false, "abc1234", "0.3.0-alpha.1", None, None, None, None,),
            ""
        );
    }
}
