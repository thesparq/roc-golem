app [main, discoverTypes, getDefinition, init, invoke] {
    pf: platform "../platform/main.roc"
}

getDefinition : {} -> Str
getDefinition = |_| "roc-counter"

discoverTypes : {} -> Str
discoverTypes = |_| "[]"

init : Str -> Try(Str, Str)
init = |_input| Ok("{\"count\":0,\"history\":[]}")

invoke : Str, Str, Str -> Try(Str, Str)
invoke = |methodName, _state, _input|
    match methodName {
        "increment" => Ok("{\"count\":1,\"history\":[\"incremented\"]}")
        _ => Err("unknown method")
    }

main = {}
