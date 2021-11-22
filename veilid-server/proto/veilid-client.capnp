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

struct VeilidState {
#   union {
       attachment       @0  :AttachmentState;
#   } 
}

struct AttachmentStateChange {
    oldState            @0  :AttachmentState;
    newState            @1  :AttachmentState;
}

struct VeilidStateChange {
#   union {
       attachment       @0 :AttachmentStateChange;
#   } 
}

interface Registration {}

interface VeilidServer {

    register @0 (veilidClient: VeilidClient) -> (registration: Registration);

    attach @1 () -> (result: Bool);
    detach @2 () -> (result: Bool);
    shutdown @3 () -> (result: Bool);

}

interface VeilidClient {

    stateChanged @0 (changed: VeilidStateChange);

}