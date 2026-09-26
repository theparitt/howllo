//! Built-in plugin registry. A database row alone cannot install code or assets.
//! Every plugin must be shipped with Howllo and added here by a code change.

pub struct BuiltInPlugin {
    pub id: &'static str,
    pub slot: &'static str,
    pub stylesheet_path: &'static str,
    pub version: &'static str,
    pub stylesheet: &'static str,
}

pub const BUILT_INS: &[BuiltInPlugin] = &[
    BuiltInPlugin {
        id: "editorial-type",
        slot: "workspace.typography",
        stylesheet_path: "/plugins/editorial-type.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/editorial-type.css"),
    },
    BuiltInPlugin {
        id: "clean-type",
        slot: "workspace.typography",
        stylesheet_path: "/plugins/clean-type.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/clean-type.css"),
    },
    BuiltInPlugin {
        id: "board-grid",
        slot: "board.directory.layout",
        stylesheet_path: "/plugins/board-grid.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/board-grid.css"),
    },
    BuiltInPlugin {
        id: "compact-topics",
        slot: "board.topics.layout",
        stylesheet_path: "/plugins/compact-topics.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/compact-topics.css"),
    },
    BuiltInPlugin {
        id: "reading-type",
        slot: "board.topics.typography",
        stylesheet_path: "/plugins/reading-type.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/reading-type.css"),
    },
];

pub fn find(id: &str) -> Option<&'static BuiltInPlugin> {
    BUILT_INS.iter().find(|plugin| plugin.id == id)
}

pub fn matches(id: &str, version: &str, slot: &str, stylesheet_path: &str) -> bool {
    find(id).is_some_and(|plugin| {
        plugin.version == version
            && plugin.slot == slot
            && plugin.stylesheet_path == stylesheet_path
            && !plugin.stylesheet.is_empty()
    })
}
