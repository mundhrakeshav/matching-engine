package entity

import (
	"encoding/json"
	"errors"
	"time"
)

type OrderKind string
type OrderSide string

const (
	OrderSideBuy  OrderSide = "Buy"
	OrderSideSell OrderSide = "Sell"
)

const (
	OrderKindMarket OrderKind = "Market"
	OrderKindLimit  OrderKind = "Limit"
)

type Order struct {
	RestingOrder
	UserID    int64           `json:"user_id"`
	Price     int64           `json:"price"`
	OrderKind OrderKind       `json:"order_kind"`
	OrderSide OrderSide       `json:"order_side"`
	Params    json.RawMessage `json:"params"`
}

type RestingOrder struct {
	ID        int64     `json:"id"`
	Quantity  int64     `json:"quantity"`
	CreatedAt time.Time `json:"created_at"`
}

type OrderLocation struct {
	Side  OrderSide // Buy → bids, Sell → asks
	Price int64     // which PriceLevel
}

func NewOrder(id, userID, price, qty int64, orderKind OrderKind, orderSide OrderSide, params json.RawMessage) *Order {
	return &Order{
		RestingOrder: *NewRestingOrder(id, qty),
		UserID:       userID,
		Price:        price,
		OrderKind:    orderKind,
		OrderSide:    orderSide,
		Params:       params,
	}
}

func NewRestingOrder(id int64, qty int64) *RestingOrder {
	return &RestingOrder{
		ID:        id,
		Quantity:  qty,
		CreatedAt: time.Now(),
	}
}

func NewOrderLocation(side OrderSide, price int64) *OrderLocation {
	return &OrderLocation{
		Side:  side,
		Price: price,
	}
}

func (k *OrderKind) Validate() error {
	if *k != OrderKindMarket && *k != OrderKindLimit {
		return errors.New("invalid order kind")
	}
	return nil
}

func (s *OrderSide) Validate() error {
	if *s != OrderSideBuy && *s != OrderSideSell {
		return errors.New("invalid order side")
	}
	return nil
}

func (o *Order) Validate() error {
	if err := o.OrderKind.Validate(); err != nil {
		return err
	}
	if err := o.OrderSide.Validate(); err != nil {
		return err
	}

	if o.Quantity <= 0 {
		return errors.New("quantity must be positive")
	}

	if o.Price <= 0 {
		return errors.New("price must be positive")
	}
	return nil
}
