package entity

import "math"

const NoneU64 = math.MaxUint64

type PriceLevel struct {
	price Price
	Qty   Quantity
	Head  NodeID
	Tail  NodeID
	// TODO: Do we even need count?
	Count uint64
}

func NewPriceLevel(price Price) *PriceLevel {
	return &PriceLevel{
		price: price,
		Head:  NoneU64,
		Tail:  NoneU64,
	}
}

func (p *PriceLevel) Less(than *PriceLevel) bool {
	return p.price < than.price
}

func (p *PriceLevel) Price() Price {
	return p.price
}
