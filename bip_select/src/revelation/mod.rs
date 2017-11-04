enum IRevealMessage {
    PeerControl(PeerControlMessage),
    ReceivedBitField(PeerInfo, BitFieldMessage),
    ReceivedHave(PeerInfo, HaveMessage),
    UpdateHave(InfoHash, HaveMessage)
}

enum ORevealMessage {
    SendBitField(PeerInfo, BitFieldMessage),
    SendHave(PeerInfo, HaveMessage)
}

trait PieceRevelation {
    
}