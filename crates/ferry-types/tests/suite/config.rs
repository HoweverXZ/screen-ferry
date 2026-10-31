use ferry_types::{Config, Role, Side};

#[test]
fn defaults_match_the_documented_config() {
    let c = Config::default();
    assert_eq!(c.schema_version, Config::SCHEMA_VERSION);
    assert_eq!(c.side, Side::Right);
    assert!(c.swap_cmd_ctrl && c.announce && c.clipboard);
    assert!(c.name.is_empty() && c.server.is_empty());
    assert_eq!(c.role, Role::native());
}
