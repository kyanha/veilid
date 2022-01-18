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

struct Attachment {
    state               @0  :AttachmentState;
}

struct VeilidUpdate {
    union {
        attachment      @0 :Attachment;
        dummy           @1 :Void;
    } 
}

struct VeilidState {
    attachment          @0 :Attachment;
}

interface Registration {}

interface VeilidServer {

    register @0 (veilidClient: VeilidClient) -> (registration: Registration);
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