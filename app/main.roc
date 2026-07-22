app [main, invoke] {
    pf: platform "../platform/main.roc"
}

# ========== Types ==========

CounterState : { count : I64 }
TodoState : { items : List(Str) }
AgentState : [Counter(CounterState), Todo(TodoState)]

# ========== Counter ==========

newCounter : CounterState
newCounter = { count: 0 }

increment : CounterState -> CounterState
increment = |s| { ..s, count: s.count + 1 }

decrement : CounterState -> CounterState
decrement = |s| { ..s, count: s.count - 1 }

counterInvoke : CounterState, Str, Str -> CounterState
counterInvoke = |self, name, _input|
    match name {
        "new" => newCounter
        "increment" => increment(self)
        "decrement" => decrement(self)
        _ => self
    }

# ========== Todo ==========

newTodo : TodoState
newTodo = { items: [] }

addItem : TodoState, Str -> TodoState
addItem = |s, item| { ..s, items: List.append(s.items, item) }

clear : TodoState -> TodoState
clear = |_| { items: [] }

todoInvoke : TodoState, Str, Str -> TodoState
todoInvoke = |self, name, input|
    match name {
        "addItem" => addItem(self, input)
        "clear" => clear(self)
        _ => self
    }

# ========== Dispatch ==========

invoke : Str, AgentState, Str -> AgentState
invoke = |name, state, input|
    match state {
        Counter(d) => Counter(counterInvoke(d, name, input))
        Todo(d) => Todo(todoInvoke(d, name, input))
    }

# ========== Metadata ==========

main : {
    name : Str,
    description : Str,
    agentNames : List(Str),
    init : AgentState,
}

main = {
    name: "roc-golem-demo",
    description: "Counter and Todo agents",
    agentNames: ["Counter", "Todo"],
    init: Counter(newCounter),
}
