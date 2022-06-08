@0xd29582d26b2fb073;

interface Registration {}

interface VeilidServer {

    register @0 (veilidClient: VeilidClient) -> (registration: Registration, state: Text);
    debug @1 (what: Text) -> (output: Text);

    attach @2 ();
    detach @3 ();
    shutdown @4 ();
    getState @5 () -> (state: Text);
}

interface VeilidClient {

    update @0 (veilidUpdate: Text);

}