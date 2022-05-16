@0xd29582d26b2fb073;

enum AttachmentState {
    detached            @0;
    attaching           @1;
    attachedWeak        @2;
    attachedGood        @3;
    attachedStrong      @4;
    fullyAttached       @5;
    overAttached        @6;
    detaching           @7;
}

struct VeilidStateAttachment {
    state               @0 :AttachmentState;
}

struct VeilidStateNetwork {
    started             @0 :Bool;
    bpsDown             @1 :UInt64;
    bpsUp               @2 :UInt64;
}

struct VeilidUpdate {
    union {
        attachment      @0 :VeilidStateAttachment;
        network         @1 :VeilidStateNetwork;
        shutdown        @2 :Void;
    } 
}

struct VeilidState {
    attachment          @0 :VeilidStateAttachment;
    network             @1 :VeilidStateNetwork;
}

interface Registration {}

interface VeilidServer {

    register @0 (veilidClient: VeilidClient) -> (registration: Registration, state: VeilidState);
    debug @1 (what: Text) -> (output: Text);

    attach @2 ();
    detach @3 ();
    shutdown @4 ();
    getState @5 () -> (state: VeilidState);
}

interface VeilidClient {

    update @0 (veilidUpdate: VeilidUpdate);
    logMessage @1 (message: Text);

}