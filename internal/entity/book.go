package entity

import "github.com/google/btree"

type Book struct {
	asks          *btree.BTreeG[*PriceLevel]
	bids          *btree.BTreeG[*PriceLevel]
	orderLocation map[int64]OrderLocation
}

func NewBook() *Book {
	return &Book{
		asks: btree.NewG(2, func(a, b *PriceLevel) bool {
			return a.Less(b)
		}),
		bids: btree.NewG(2, func(a, b *PriceLevel) bool {
			return b.Less(a)
		}),
		orderLocation: make(map[int64]OrderLocation),
	}
}
