//! Built-in plugin registry. A database row alone cannot install code or assets.
//! Every plugin must be shipped with Howllo and added here by a code change.

pub struct BuiltInPlugin {
    pub id: &'static str,
    pub slot: &'static str,
    pub stylesheet_path: &'static str,
    pub version: &'static str,
    pub stylesheet: &'static str,
    pub runtime_kind: &'static str,
    pub capabilities: &'static [&'static str],
}

pub const BUILT_INS: &[BuiltInPlugin] = &[
    BuiltInPlugin {
        id: "editorial-type",
        slot: "workspace.typography",
        stylesheet_path: "/plugins/editorial-type.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/editorial-type.css"),
        runtime_kind: "style",
        capabilities: &["workspace.typography"],
    },
    BuiltInPlugin {
        id: "clean-type",
        slot: "workspace.typography",
        stylesheet_path: "/plugins/clean-type.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/clean-type.css"),
        runtime_kind: "style",
        capabilities: &["workspace.typography"],
    },
    BuiltInPlugin {
        id: "board-grid",
        slot: "board.directory.layout",
        stylesheet_path: "/plugins/board-grid.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/board-grid.css"),
        runtime_kind: "style",
        capabilities: &["board.directory.layout"],
    },
    BuiltInPlugin {
        id: "compact-topics",
        slot: "board.topics.layout",
        stylesheet_path: "/plugins/compact-topics.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/compact-topics.css"),
        runtime_kind: "style",
        capabilities: &["board.topics.layout"],
    },
    BuiltInPlugin {
        id: "reading-type",
        slot: "board.topics.typography",
        stylesheet_path: "/plugins/reading-type.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/reading-type.css"),
        runtime_kind: "style",
        capabilities: &["board.topics.typography"],
    },
    BuiltInPlugin {
        id: "simple-markdown",
        slot: "board.post.markdown",
        stylesheet_path: "/plugins/simple-markdown.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/simple-markdown.css"),
        runtime_kind: "component",
        capabilities: &["post.editor", "post.body"],
    },
    BuiltInPlugin {
        id: "pdf-preview",
        slot: "board.attachments.pdf",
        stylesheet_path: "/plugins/pdf-preview.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/pdf-preview.css"),
        runtime_kind: "component",
        capabilities: &["attachment.upload.pdf", "attachment.preview.pdf"],
    },
    BuiltInPlugin {
        id: "model-preview",
        slot: "board.attachments.model",
        stylesheet_path: "/plugins/model-preview.css",
        version: "1.0.0",
        stylesheet: include_str!("../../../howllo-web/public/plugins/model-preview.css"),
        runtime_kind: "component",
        capabilities: &["attachment.upload.glb", "attachment.preview.glb"],
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
