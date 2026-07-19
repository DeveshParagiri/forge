//! Personal welcome-screen branding.
//!
//! Keep the fork-specific policy in this small leaf module so upstream welcome
//! layout changes can be rebased without repeatedly editing their structure.

pub(super) const PRODUCT_NAME: &str = "Exaforge";
pub(super) const SUBTITLE: Option<&str> = None;
const SHOW_REMOTE_ANNOUNCEMENTS: bool = false;
const SHOW_STARTUP_CHANGELOG: bool = false;

pub(super) fn announcement(
    value: Option<&xai_grok_announcements::RemoteAnnouncement>,
) -> Option<&xai_grok_announcements::RemoteAnnouncement> {
    SHOW_REMOTE_ANNOUNCEMENTS.then_some(value).flatten()
}

pub(super) fn changelog_bullets(value: &[String]) -> &[String] {
    if SHOW_STARTUP_CHANGELOG { value } else { &[] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_welcome_contract_is_stable() {
        let announcement = xai_grok_announcements::RemoteAnnouncement::default();
        assert_eq!(PRODUCT_NAME, "Exaforge");
        assert_eq!(SUBTITLE, None);
        assert!(super::announcement(Some(&announcement)).is_none());
        assert!(super::changelog_bullets(&["release note".into()]).is_empty());
    }
}
