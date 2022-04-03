@0x8ffce8033734ab02;

# IDs And Hashes
##############################

struct Curve25519PublicKey {
    u0                      @0  :UInt64;
    u1                      @1  :UInt64;
    u2                      @2  :UInt64;
    u3                      @3  :UInt64;
}

struct Ed25519Signature {
    u0                      @0  :UInt64;
    u1                      @1  :UInt64;
    u2                      @2  :UInt64;
    u3                      @3  :UInt64;
    u4                      @4  :UInt64;
    u5                      @5  :UInt64;
    u6                      @6  :UInt64;
    u7                      @7  :UInt64;
}

struct XChaCha20Poly1305Nonce {
    u0                      @0  :UInt64;
    u1                      @1  :UInt64;
    u2                      @2  :UInt64;
}

struct BLAKE3Hash {
    u0                      @0  :UInt64;
    u1                      @1  :UInt64;
    u2                      @2  :UInt64;
    u3                      @3  :UInt64;
}

using NodeID = Curve25519PublicKey;
using RoutePublicKey = Curve25519PublicKey;
using ValueID = Curve25519PublicKey;
using Nonce = XChaCha20Poly1305Nonce;
using Signature = Ed25519Signature;
using BlockID = BLAKE3Hash;
using TunnelID = UInt64;

# Node Dial Info
################################################################

struct AddressIPV4 {
    addr                    @0  :UInt32;                # Address in big endian format
}

struct AddressIPV6 {
    addr0                   @0  :UInt32;                # \ 
    addr1                   @1  :UInt32;                #  \ Address in big 
    addr2                   @2  :UInt32;                #  / endian format
    addr3                   @3  :UInt32;                # / 
}

struct Address {
    union {
        ipv4                @0  :AddressIPV4;
        ipv6                @1  :AddressIPV6;
    }
}

struct SocketAddress {
    address                 @0  :Address;
    port                    @1  :UInt16;
}

enum ProtocolKind {
    udp                     @0;
    ws                      @1;
    wss                     @2;
    tcp                     @3;
}

struct DialInfoUDP {
    socketAddress           @0  :SocketAddress;
}

struct DialInfoTCP {
    socketAddress           @0  :SocketAddress;
}

struct DialInfoWS {
    socketAddress           @0  :SocketAddress;
    request                 @1  :Text;
}

struct DialInfoWSS {
    socketAddress           @0  :SocketAddress;
    request                 @1  :Text;
}

struct DialInfo {
    union {
        udp                 @0  :DialInfoUDP;
        tcp                 @1  :DialInfoTCP;
        ws                  @2  :DialInfoWS;
        wss                 @3  :DialInfoWSS;
    }
}

struct NodeDialInfo {
    nodeId                  @0  :NodeID;                # node id
    dialInfo                @1  :DialInfo;              # how to get to the node
}

# Private Routes
##############################

struct RouteHopData {         
    nonce                   @0  :Nonce;                 # nonce for encrypted blob
    blob                    @1  :Data;                  # encrypted blob with ENC(nonce,DH(PK,SK))
                                                        #   can be one of: 
                                                        #     if more hops remain in this route: RouteHop (0 byte appended as key)
                                                        #     if end of safety route and starting private route: PrivateRoute (1 byte appended as key)
}

struct RouteHop {
    dialInfo                @0  :NodeDialInfo;          # dial info for this hop
    nextHop                 @1  :RouteHopData;          # Optional: next hop in encrypted blob 
                                                        # Null means no next hop, at destination (only used in private route, safety routes must enclose a stub private route)
}

struct PrivateRoute {
    publicKey               @0  :RoutePublicKey;        # private route public key (unique per private route)
    hopCount                @1  :UInt8;                 # Count of hops left in the private route
    firstHop                @2  :RouteHop;              # Optional: first hop in the private route
}

struct SafetyRoute {
    publicKey               @0  :RoutePublicKey;        # safety route public key (unique per safety route)
    hopCount                @1  :UInt8;                 # Count of hops left in the safety route
    hops :union {
        data                @2  :RouteHopData;          # safety route has more hops
        private             @3  :PrivateRoute;          # safety route has ended and private route follows
    }
}

# Values
##############################

using ValueSeqNum = UInt32;                             # sequence numbers for values

struct ValueKey {
    publicKey               @0  :ValueID;               # the location of the value
    subkey                  @1  :Text;                  # the name of the subkey (or empty if the whole key)
}

struct ValueKeySeq {
    key                     @0  :ValueKey;              # the location of the value
    seq                     @1  :ValueSeqNum;           # the sequence number of the value subkey
}

struct ValueData {
    data                    @0  :Data;                  # value or subvalue contents in CBOR format
    seq                     @1  :ValueSeqNum;           # sequence number of value
}

# Operations
##############################

struct OperationInfoQ {
    nodeInfo                @0  :NodeInfo;              # node info update about the infoq sender
}


enum NetworkClass {
    server                  @0;                         # S = Device with public IP and no UDP firewall
    mapped                  @1;                         # M = Device with portmap behind any NAT
    fullConeNAT             @2;                         # F = Device without portmap behind full-cone NAT
    addressRestrictedNAT    @3;                         # R1 = Device without portmap behind address-only restricted NAT
    portRestrictedNAT       @4;                         # R2 = Device without portmap behind address-and-port restricted NAT
    outboundOnly            @5;                         # O = Outbound only
    webApp                  @6;                         # W = PWA in either normal or tor web browser
    invalid                 @7;                         # X = Invalid
}

struct NodeInfo {
    networkClass            @0  :NetworkClass;
    willRoute               @1  :Bool;
    willTunnel              @2  :Bool;
    willSignal              @3  :Bool;
    willRelay               @4  :Bool;
    willValidateDialInfo    @5  :Bool;
}

struct SenderInfo {
    socketAddress           @0  :SocketAddress;         # socket address was available for peer
}

struct OperationInfoA {
    nodeInfo                @0  :NodeInfo;              # returned node information
    senderInfo              @1  :SenderInfo;            # info about InfoQ sender from the perspective of the replier
}

struct OperationValidateDialInfo {
    dialInfo                @0  :DialInfo;              # dial info to use for the receipt
    receipt                 @1  :Data;                  # receipt to return to dial info to prove it is reachable
    redirect                @2  :Bool;                  # request a different node do the validate
    alternatePort           @3  :Bool;                  # return receipt from a different source port than the default
}

struct OperationReturnReceipt {
    receipt                 @0  :Data;                  # receipt being returned to its origin
}

struct OperationFindNodeQ {    
    nodeId                  @0  :NodeID;                # node id to locate
    dialInfoList            @1  :List(DialInfo);        # dial info for the node asking the question
}

struct PeerInfo {
    nodeId                  @0  :NodeID;                # node id or 'closer peer'
    dialInfoList            @1  :List(DialInfo);        # dial info for 'closer peer'
}

struct OperationFindNodeA {
    peers                   @0  :List(PeerInfo);        # returned 'closer peer' information
}

struct RoutedOperation {
    signatures              @0  :List(Signature);       # signatures from nodes that have handled the private route
    nonce                   @1  :Nonce;                 # nonce Xmsg 
    data                    @2  :Data;                  # Operation encrypted with ENC(Xmsg,DH(PKapr,SKbsr))
}

struct OperationRoute {
    safetyRoute             @0  :SafetyRoute;           # Where this should go
    operation               @1  :RoutedOperation;       # The operation to be routed
}

struct OperationGetValueQ {
    key                     @0  :ValueKey;              # key for value to get
}

struct OperationGetValueA {
    union {
        data                @0  :ValueData;             # the value if successful
        peers               @1  :List(PeerInfo);        # returned 'closer peer' information if not successful       
    }
}

struct OperationSetValueQ {
    key                     @0  :ValueKey;              # key for value to update
    value                   @1  :ValueData;             # value or subvalue contents in CBOR format (older or equal seq number gets dropped)
}

struct OperationSetValueA {
    union {
        data                @0  :ValueData;             # the new value if successful, may be a different value than what was set if the seq number was lower or equal
        peers               @1  :List(PeerInfo);        # returned 'closer peer' information if not successful       
    }
}

struct OperationWatchValueQ {
    key                     @0  :ValueKey;              # key for value to watch
}

struct OperationWatchValueA {
    expiration              @0  :UInt64;                # timestamp when this watch will expire in usec since epoch (0 if watch failed)
    peers                   @1  :List(PeerInfo);        # returned list of other nodes to ask that could propagate watches
}

struct OperationValueChanged {
    key                     @0  :ValueKey;              # key for value that changed
    value                   @1  :ValueData;             # value or subvalue contents in CBOR format with sequence number
}

struct OperationSupplyBlockQ {
    blockId                 @0  :BlockID;               # hash of the block we can supply
}

struct OperationSupplyBlockA {
    union {
        expiration          @0  :UInt64;                # when the block supplier entry will need to be refreshed
        peers               @1  :List(PeerInfo);        # returned 'closer peer' information if not successful       
    }
}

struct OperationFindBlockQ {
    blockId                 @0  :BlockID;               # hash of the block we can supply
}

struct OperationFindBlockA {
    data                    @0  :Data;                  # Optional: the actual block data if we have that block ourselves
                                                        # null if we don't have a block to return
    suppliers               @1  :List(PeerInfo);        # returned list of suppliers if we have them
    peers                   @2  :List(PeerInfo);        # returned 'closer peer' information 
}

struct OperationSignalQ {
    data                    @0  :Data;                  # the signalling system request
}

struct OperationSignalA {
    data                    @0  :Data;                  # the signalling system response
}

enum TunnelEndpointMode {
    raw                     @0;                         # raw tunnel
    turn                    @1;                         # turn tunnel
}

enum TunnelError {
    badId                   @0;                         # Tunnel ID was rejected
    noEndpoint              @1;                         # Endpoint was unreachable
    rejectedMode            @2;                         # Endpoint couldn't provide mode
    noCapacity              @3;                         # Endpoint is full
}

struct TunnelEndpoint {
    nodeId                  @0  :NodeID;                # node id
    dialInfoList            @1  :List(DialInfo);        # how to reach the node
    mode                    @2  :TunnelEndpointMode;    # what kind of endpoint this is
}

struct FullTunnel {
    id                      @0  :TunnelID;              # tunnel id to use everywhere
    timeout                 @1  :UInt64;                # duration from last data when this expires if no data is sent or received
    local                   @2  :TunnelEndpoint;        # local endpoint
    remote                  @3  :TunnelEndpoint;        # remote endpoint
}

struct PartialTunnel {
    id                      @0  :TunnelID;              # tunnel id to use everywhere
    timeout                 @1  :UInt64;                # timestamp when this expires if not completed
    local                   @2  :TunnelEndpoint;        # local endpoint
}

struct OperationStartTunnelQ {
    id                      @0  :TunnelID;              # tunnel id to use everywhere
    localMode               @1  :TunnelEndpointMode;    # what kind of local endpoint mode is being requested
    depth                   @2  :UInt8;                 # the number of nodes in the tunnel
}

struct OperationStartTunnelA {
    union {
        partial             @0  :PartialTunnel;         # the first half of the tunnel
        error               @1  :TunnelError;           # if we didn't start the tunnel, why not
    }
}

struct OperationCompleteTunnelQ {
    id                      @0  :TunnelID;              # tunnel id to use everywhere
    localMode               @1  :TunnelEndpointMode;    # what kind of local endpoint mode is being requested
    depth                   @2  :UInt8;                 # the number of nodes in the tunnel
    endpoint                @3  :TunnelEndpoint;        # the remote endpoint to complete
}

struct OperationCompleteTunnelA {
    union {
        tunnel              @0  :FullTunnel;            # the tunnel description
        error               @1  :TunnelError;           # if we didn't complete the tunnel, why not
    }
}

struct OperationCancelTunnelQ {
    tunnel                  @0  :TunnelID;              # the tunnel id to cancel
}

struct OperationCancelTunnelA {
    union {
        tunnel              @0  :TunnelID;              # the tunnel id that was cancelled
        error               @1  :TunnelError;           # if we couldn't cancel, why not
    }
}

struct Operation {
    opId                    @0  :UInt64;                # Random RPC ID. Must be random to foil reply forgery attacks. 

    respondTo :union {
        none                @1  :Void;                  # no response is desired
        sender              @2  :DialInfo;              # (Optional) the 'best' envelope-sender dial info to be used for reply (others may exist via findNodeQ)
        privateRoute        @3  :PrivateRoute;          # embedded private route to be used for reply
    }                              

    detail :union {
        # Direct operations
        infoQ               @4  :OperationInfoQ;
        infoA               @5  :OperationInfoA;
        validateDialInfo    @6  :OperationValidateDialInfo;
        findNodeQ           @7  :OperationFindNodeQ;
        findNodeA           @8  :OperationFindNodeA;
        route               @9  :OperationRoute;
        
        # Routable operations
        getValueQ           @10 :OperationGetValueQ;
        getValueA           @11 :OperationGetValueA;
        setValueQ           @12 :OperationSetValueQ;
        setValueA           @13 :OperationSetValueA;
        watchValueQ         @14 :OperationWatchValueQ;
        watchValueA         @15 :OperationWatchValueA;
        valueChanged        @16 :OperationValueChanged;
        
        supplyBlockQ        @17 :OperationSupplyBlockQ;
        supplyBlockA        @18 :OperationSupplyBlockA; 
        findBlockQ          @19 :OperationFindBlockQ;
        findBlockA          @20 :OperationFindBlockA; 
    
        signalQ             @21 :OperationSignalQ;
        signalA             @22 :OperationSignalA;
        
        returnReceipt       @23 :OperationReturnReceipt;
        
        # Tunnel operations
        startTunnelQ        @24 :OperationStartTunnelQ;
        startTunnelA        @25 :OperationStartTunnelA;
        completeTunnelQ     @26 :OperationCompleteTunnelQ;
        completeTunnelA     @27 :OperationCompleteTunnelA;
        cancelTunnelQ       @28 :OperationCancelTunnelQ; 
        cancelTunnelA       @29 :OperationCancelTunnelA;
    }
}
