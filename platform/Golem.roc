module [
    Agent,
    defineAgent,
    logTrace,
    logDebug,
    logInfo,
    logWarn,
    logError,
    workerId,
    rpc,
    setPersistence,
    http,
    websocketConnect,
    websocketSend,
    websocketReceive,
    websocketClose,
    sleep,
    now,
]

import Types exposing [
    Metadata,
    ToolCall,
    ToolResult,
    PersistenceMode,
]
import Effect

## Represents a complete Golem Agent specification
Agent state : {
    init : Str -> Result state Str,
    handleMessage : state, Str -> Result { state : state, response : Str } Str,
    handleToolCall : state, ToolCall -> Result { state : state, result : ToolResult } Str,
    metadata : Metadata,
    serializeState : state -> Str,
    deserializeState : Str -> Result state Str,
}

## Define a new Golem Agent
defineAgent :
    {
        init : Str -> Result state Str,
        handleMessage : state, Str -> Result { state : state, response : Str } Str,
        handleToolCall : state, ToolCall -> Result { state : state, result : ToolResult } Str,
        metadata : Metadata,
        serializeState : state -> Str,
        deserializeState : Str -> Result state Str,
    }
    -> Agent state
defineAgent = |config| config

## Logging functions
logTrace : Str -> Effect.Effect {}
logTrace = |msg| Effect.log Trace msg

logDebug : Str -> Effect.Effect {}
logDebug = |msg| Effect.log Debug msg

logInfo : Str -> Effect.Effect {}
logInfo = |msg| Effect.log Info msg

logWarn : Str -> Effect.Effect {}
logWarn = |msg| Effect.log Warn msg

logError : Str -> Effect.Effect {}
logError = |msg| Effect.log Error msg

## Get current Golem Worker ID
workerId : Effect.Effect Str
workerId = Effect.getWorkerId

## Remote Procedure Call to another Golem Worker
rpc : Str, Str, Str -> Effect.Effect (Result Str Str)
rpc = |targetWorker, fnName, payload|
    Effect.rpcInvoke targetWorker fnName payload

## Set Golem Persistence Mode
setPersistence : PersistenceMode -> Effect.Effect {}
setPersistence = |mode|
    Effect.setPersistenceMode mode

## Perform outgoing HTTP request
http : Str -> Effect.Effect (Result Str Str)
http = |reqJson|
    Effect.httpRequest reqJson

## Connect to a WebSocket endpoint
websocketConnect : Str -> Effect.Effect (Result U32 Str)
websocketConnect = |url|
    Effect.wsConnect url

## Send text message over WebSocket connection
websocketSend : U32, Str -> Effect.Effect (Result {} Str)
websocketSend = |handle, msg|
    Effect.wsSend handle msg

## Receive text message from WebSocket connection
websocketReceive : U32 -> Effect.Effect (Result Str Str)
websocketReceive = |handle|
    Effect.wsReceive handle

## Close WebSocket connection
websocketClose : U32 -> Effect.Effect (Result {} Str)
websocketClose = |handle|
    Effect.wsClose handle

## Durable sleep for specified milliseconds
sleep : U64 -> Effect.Effect {}
sleep = |millis|
    Effect.sleepMillis millis

## Get current timestamp in milliseconds
now : Effect.Effect U64
now = Effect.nowMillis
