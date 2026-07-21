package entity

import "time"

type Trade struct {
	ID                         TradeID
	Sequence                   SequenceNumber
	InstrumentID               InstrumentID
	MakerID, TakerID           UserID
	Qty                        Quantity
	Price                      Price
	MakerOrderID, TakerOrderID OrderID
	TakerSide                  OrderSideType
	CreatedAt                  time.Time
}
