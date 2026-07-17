package app

import "ob/internal/entity"

type Engine interface {
	SubmitOrder(order *entity.Order) error
	// GetOrder(id int64) (*entity.Order, error)
	// UpdateOrder(id int64, order *entity.Order) error
	// DeleteOrder(id int64) error
}
