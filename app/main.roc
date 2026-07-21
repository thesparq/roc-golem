app [main, getAgentType, initialize, invoke, discoverTypes, save, load] {
    pf: platform "../platform/main.roc"
}

getAgentType : Str -> Str
getAgentType = |typeName| typeName

initialize : Str, Str -> I32
initialize = |_, _| 0

invoke : Str, Str -> Str
invoke = |methodName, _input| methodName

discoverTypes : {} -> Str
discoverTypes = |_| "[]"

save : {} -> Str
save = |_| "{}"

load : Str -> I32
load = |_| 0

main = {}
