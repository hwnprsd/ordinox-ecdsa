package thresholdecdsa

type EvmTransferMessage struct {
	ToAddress    string
	TokenAddress string
	Amount       string
	ChainId      string
	Nonce        uint64
}

func NewEvmTransferMessage(nonce uint64, chainId, toAddr, tokenAddr, amount string) EvmTransferMessage {
	return EvmTransferMessage{
		ToAddress:    toAddr,
		TokenAddress: tokenAddr,
		Amount:       amount,
		Nonce:        nonce,
		ChainId:      chainId,
	}
}
