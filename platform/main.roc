platform "roc-golem"
    requires {
        main : {},
        discoverTypes : {} -> Str,
        getDefinition : {} -> Str,
        init : Str -> Try(Str, Str),
        invoke : Str, Str, Str -> Try(Str, Str),
    }
    exposes []
    packages {}
    provides {
        "roc_get_agent_type": get_agent_type!,
        "roc_initialize":     initialize!,
        "roc_invoke":         invoke!,
        "roc_discover_types": discover_types!,
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
            import_memory: Zeroed,
            initial_stack_size: 14752,
            minimum_memory: 262144,
        },
    }

get_agent_type! : Str -> Str
get_agent_type! = |_typeName| getDefinition({})

initialize! : Str, Str -> Str
initialize! = |_agentType, input| match init(input) {
    Ok(state) => state
    Err(_) => ""
}

invoke! : Str, Str, Str -> Str
invoke! = |methodName, state, input| match invoke(methodName, state, input) {
    Ok(result) => result
    Err(_) => "{}"
}

discover_types! : {} -> Str
discover_types! = |_| discoverTypes({})

main! : {} -> I32
main! = |_| 0
