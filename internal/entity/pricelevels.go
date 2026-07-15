package entity

type PriceLevel struct {
	Price  int64
	Orders []*RestingOrder
}

func NewPriceLevel(price int64) *PriceLevel {
	return &PriceLevel{
		Price:  price,
		Orders: make([]*RestingOrder, 0),
	}
}

func (p *PriceLevel) Less(than *PriceLevel) bool {
	return p.Price < than.Price
}
