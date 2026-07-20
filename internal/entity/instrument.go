package entity

import "time"

type Instrument struct {
	ID         InstrumentID `json:"id"`
	TickSize   Price        `json:"tick_size"`
	LotSize    Quantity     `json:"lot_size"`
	Symbol     string       `json:"symbol"`
	BaseAsset  string       `json:"base_asset"`
	QuoteAsset string       `json:"quote_asset"`
	ExpiryDate time.Time    `json:"expiry_date"`
	CreatedAt  time.Time    `json:"created_at"`
}

func NewInstrument(id InstrumentID, tickSize Price, lotSize Quantity, symbol string, baseAsset string, quoteAsset string, expiryDate time.Time) *Instrument {
	return &Instrument{
		ID:         id,
		TickSize:   tickSize,
		LotSize:    lotSize,
		Symbol:     symbol,
		BaseAsset:  baseAsset,
		QuoteAsset: quoteAsset,
		ExpiryDate: expiryDate,
		CreatedAt:  time.Now(),
	}
}
