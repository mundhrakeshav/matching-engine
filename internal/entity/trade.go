package entity

import "time"

type Trade struct {
	ID                         TradeID
	MakerID, TakerID           uint64
	Qty                        Quantity
	Price                      Price
	MakerOrderID, TakerOrderID OrderID
	TakerSide                  OrderSideType
	CreatedAt                  time.Time
}
