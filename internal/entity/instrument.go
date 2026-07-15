package entity

import "time"

type Instrument struct {
	ID         int64     `json:"id"`
	TickSize   int64     `json:"tick_size"`
	LotSize    int64     `json:"lot_size"`
	Symbol     string    `json:"symbol"`
	BaseAsset  string    `json:"base_asset"`
	QuoteAsset string    `json:"quote_asset"`
	ExpiryDate time.Time `json:"expiry_date"`
	CreatedAt  time.Time `json:"created_at"`
}

func NewInstrument(id int64, tickSize int64, lotSize int64, symbol string, baseAsset string, quoteAsset string, expiryDate time.Time) *Instrument {
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
