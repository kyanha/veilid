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

# Signals
##############################

struct SignalInfoHolePunch {
    receipt                 @0  :Data;                  # receipt to return with hole punch
    peerInfo                @1  :PeerInfo;              # peer info of the signal sender for hole punch attempt
}

struct SignalInfoReverseConnect {
    receipt                 @0  :Data;                  # receipt to return with reverse connect
    peerInfo                @1  :PeerInfo;              # peer info of the signal sender for reverse connect attempt
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

struct OperationStatusQ {
    nodeStatus              @0  :NodeStatus;            # node status update about the statusq sender
}

enum NetworkClass {
    inboundCapable          @0;                         # I = Inbound capable without relay, may require signal
    outboundOnly            @1;                         # O = Outbound only, inbound relay required except with reverse connect signal
    webApp                  @2;                         # W = PWA, outbound relay is required in most cases
}

enum DialInfoClass {
    direct                  @0;                         # D = Directly reachable with public IP and no firewall, with statically configured port
    mapped                  @1;                         # M = Directly reachable with via portmap behind any NAT or firewalled with dynamically negotiated port
    fullConeNAT             @2;                         # F = Directly reachable device without portmap behind full-cone NAT
    blocked                 @3;                         # B = Inbound blocked at firewall but may hole punch with public address
    addressRestrictedNAT    @4;                         # A = Device without portmap behind address-only restricted NAT
    portRestrictedNAT       @5;                         # P = Device without portmap behind address-and-port restricted NAT
}

struct DialInfoDetail {
    dialInfo                @0  :DialInfo;
    class                   @1  :DialInfoClass;
}

struct NodeStatus {
    willRoute               @0  :Bool;
    willTunnel              @1  :Bool;
    willSignal              @2  :Bool;
    willRelay               @3  :Bool;
    willValidateDialInfo    @4  :Bool;
}

struct ProtocolSet {
    udp                     @0  :Bool;
    tcp                     @1  :Bool;
    ws                      @2  :Bool;
    wss                     @3  :Bool;
}

struct NodeInfo {
    networkClass            @0  :NetworkClass;          # network class of this node
    outboundProtocols       @1  :ProtocolSet;           # protocols that can go outbound
    minVersion              @2  :UInt8;                 # minimum protocol version for rpc
    maxVersion              @3  :UInt8;                 # maximum protocol version for rpc
    dialInfoDetailList      @4  :List(DialInfoDetail);  # inbound dial info details for this node
    relayPeerInfo           @5  :PeerInfo;              # (optional) relay peer info for this node
}

struct SignedNodeInfo {
    nodeInfo                @0  :NodeInfo;              # node info
    signature               @1  :Signature;             # signature
    timestamp               @2  :UInt64;                # when signed node info was generated
}

struct SenderInfo {
    socketAddress           @0  :SocketAddress;         # socket address was available for peer
}

struct OperationStatusA {
    nodeStatus              @0  :NodeStatus;            # returned node status
    senderInfo              @1  :SenderInfo;            # info about StatusQ sender from the perspective of the replier
}

struct OperationValidateDialInfo {
    dialInfo                @0  :DialInfo;              # dial info to use for the receipt
    receipt                 @1  :Data;                  # receipt to return to dial info to prove it is reachable
    redirect                @2  :Bool;                  # request a different node do the validate
}

struct OperationReturnReceipt {
    receipt                 @0  :Data;                  # receipt being returned to its origin
}

struct OperationFindNodeQ {    
    nodeId                  @0  :NodeID;                # node id to locate
}

struct PeerInfo {
    nodeId                  @0  :NodeID;                # node id for 'closer peer'
    signedNodeInfo          @1  :SignedNodeInfo;        # signed node info for 'closer peer'
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

struct OperationNodeInfoUpdate {
    signedNodeInfo          @0  :SignedNodeInfo;        # Our signed node info
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

struct OperationSignal {
    union {
        holePunch           @0  :SignalInfoHolePunch;
        reverseConnect      @1  :SignalInfoReverseConnect;
    }
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
    mode                    @0  :TunnelEndpointMode;    # what kind of endpoint this is
    peerInfo                @1  :PeerInfo;                # node id and dialinfo
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
        sender              @2  :Void;                  # sender without node info
        senderWithInfo      @3  :SignedNodeInfo;        # some envelope-sender signed node info to be used for reply
        privateRoute        @4  :PrivateRoute;          # embedded private route to be used for reply
    }                              

    detail :union {
        # Direct operations
        statusQ             @5  :OperationStatusQ;
        statusA             @6  :OperationStatusA;
        validateDialInfo    @7  :OperationValidateDialInfo;
        findNodeQ           @8  :OperationFindNodeQ;
        findNodeA           @9  :OperationFindNodeA;
        route               @10 :OperationRoute;
        nodeInfoUpdate      @11 :OperationNodeInfoUpdate;
        
        # Routable operations
        getValueQ           @12 :OperationGetValueQ;
        getValueA           @13 :OperationGetValueA;
        setValueQ           @14 :OperationSetValueQ;
        setValueA           @15 :OperationSetValueA;
        watchValueQ         @16 :OperationWatchValueQ;
        watchValueA         @17 :OperationWatchValueA;
        valueChanged        @18 :OperationValueChanged;
        
        supplyBlockQ        @19 :OperationSupplyBlockQ;
        supplyBlockA        @20 :OperationSupplyBlockA; 
        findBlockQ          @21 :OperationFindBlockQ;
        findBlockA          @22 :OperationFindBlockA; 
    
        signal              @23 :OperationSignal;
        returnReceipt       @24 :OperationReturnReceipt;
        
        # Tunnel operations
        startTunnelQ        @25 :OperationStartTunnelQ;
        startTunnelA        @26 :OperationStartTunnelA;
        completeTunnelQ     @27 :OperationCompleteTunnelQ;
        completeTunnelA     @28 :OperationCompleteTunnelA;
        cancelTunnelQ       @29 :OperationCancelTunnelQ; 
        cancelTunnelA       @30 :OperationCancelTunnelA;
    }
}
