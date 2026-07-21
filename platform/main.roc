platform "roc-golem"
    requires { main : {}, getAgentType : Str -> Str, initialize : Str, Str -> I32, invoke : Str, Str -> Str, discoverTypes : {} -> Str, save : {} -> Str, load : Str -> I32 }
    exposes []
    packages {}
    provides {
        "roc_get_agent_type": get_agent_type!,
        "roc_initialize":     initialize!,
        "roc_invoke":         invoke!,
        "roc_discover_types": discover_types!,
        "roc_save":           save!,
        "roc_load":           load!,
    }
    hosted {}
    targets: {
        inputs_dir: "targets/",
        wasm32: {
            inputs: ["host.wasm", app],
            output: Shared,
            exports: [
                "roc_get_agent_type",
                "roc_initialize",
                "roc_invoke",
                "roc_discover_types",
                "roc_save",
                "roc_load",
                "golem:agent/guest@1.5.0#initialize",
                "golem:agent/guest@1.5.0#invoke",
                "golem:agent/guest@1.5.0#get-definition",
                "golem:agent/guest@1.5.0#discover-agent-types",
                "golem:api/save-snapshot@1.5.0#save",
                "golem:api/load-snapshot@1.5.0#load",
                "cabi_realloc",
                "cabi_post_golem:agent/guest@1.5.0#initialize",
                "cabi_post_golem:agent/guest@1.5.0#invoke",
                "cabi_post_golem:agent/guest@1.5.0#get-definition",
                "cabi_post_golem:agent/guest@1.5.0#discover-agent-types",
                "cabi_post_golem:api/save-snapshot@1.5.0#save",
                "cabi_post_golem:api/load-snapshot@1.5.0#load",
            ],
            import_memory: Zeroed,
            initial_stack_size: 14752,
            minimum_memory: 65536,
        },
    }

get_agent_type! : Str -> Str
get_agent_type! = |type_name| getAgentType(type_name)

initialize! : Str, Str -> I32
initialize! = |agent_type, input| initialize(agent_type, input)

invoke! : Str, Str -> Str
invoke! = |method_name, input| invoke(method_name, input)

discover_types! : {} -> Str
discover_types! = |_| discoverTypes({})

save! : {} -> Str
save! = |_| save({})

load! : Str -> I32
load! = |snapshot| load(snapshot)

main! : {} -> I32
main! = |_| 0
