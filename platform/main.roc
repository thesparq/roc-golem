platform "roc-golem"
    requires {
        main : {
            name : Str,
            description : Str,
            agentNames : List(Str),
            init : [Counter({ count : I64 }), Todo({ items : List(Str) })],
        },
        invoke : Str, [Counter({ count : I64 }), Todo({ items : List(Str) })], Str -> [Counter({ count : I64 }), Todo({ items : List(Str) })],
    }
    exposes []
    packages {}
    provides {
        "roc_golem_initialize":        roc_golem_initialize!,
        "roc_golem_invoke":            roc_golem_invoke!,
        "roc_golem_get_definition":    roc_golem_get_definition!,
        "roc_golem_discover_types":    roc_golem_discover_types!,
        "roc_golem_save":              roc_golem_save!,
        "roc_golem_load":              roc_golem_load!,
    }
    hosted {}
    targets: {
        inputs_dir: "targets/",
        wasm32: {
            inputs: ["host.wasm", app],
            output: Shared,
            exports: [
                "roc_golem_initialize",
                "roc_golem_invoke",
                "roc_golem_get_definition",
                "roc_golem_discover_types",
                "roc_golem_save",
                "roc_golem_load",
                "golem:agent/guest@1.5.0#initialize",
                "golem:agent/guest@1.5.0#invoke",
                "golem:agent/guest@1.5.0#get-definition",
                "golem:agent/guest@1.5.0#discover-agent-types",
                "golem:api/save-snapshot@1.5.0#save",
                "golem:api/load-snapshot@1.5.0#load",
                "cabi_realloc",
                "roc_dealloc",
                "cabi_post_golem:agent/guest@1.5.0#initialize",
                "cabi_post_golem:agent/guest@1.5.0#invoke",
                "cabi_post_golem:agent/guest@1.5.0#get-definition",
                "cabi_post_golem:agent/guest@1.5.0#discover-agent-types",
                "cabi_post_golem:api/save-snapshot@1.5.0#save",
                "cabi_post_golem:api/load-snapshot@1.5.0#load",
            ],
            initial_stack_size: 14752,
            minimum_memory: 262144,
        },
    }

# ── Agent metadata helpers ──
# Pure Roc functions at JSON/Str level.
# WIT binary encoding stays in Rust (Roc Str can't hold 0x00 bytes).

build_agent_metadata : Str, Str -> Str
build_agent_metadata = |name, description|
    Str.concat(
        "{ \"typeName\": \"",
        Str.concat(name,
            Str.concat("\", \"description\": \"",
                Str.concat(description,
                    "\", \"sourceLanguage\": \"roc\", \"mode\": \"ephemeral\", \"snapshotting\": \"enabled(default)\" }"
                )
            )
        )
    )

# ── Golem protocol handlers ──
# Receive/return Str (Roc len-prefixed).
# JSON parse stays here (needs AgentState type from requires {}).

roc_golem_initialize! : Str, Str -> Str
roc_golem_initialize! = |_agentType, _input|
    Encoding.Json.to_str(main.init)

roc_golem_invoke! : Str, Str, Str -> Str
roc_golem_invoke! = |methodName, stateJson, input|
    match Encoding.Json.parse(stateJson) {
        Ok(state) => Encoding.Json.to_str(invoke(methodName, state, input))
        Err(_) => "{}"
    }

roc_golem_get_definition! : {} -> Str
roc_golem_get_definition! = |_|
    build_agent_metadata(main.name, main.description)

roc_golem_discover_types! : {} -> Str
roc_golem_discover_types! = |_|
    ""

roc_golem_save! : Str -> Str
roc_golem_save! = |stateJson|
    stateJson

roc_golem_load! : Str -> Str
roc_golem_load! = |snapshotJson|
    snapshotJson

main! : {} -> I32
main! = |_| 0
