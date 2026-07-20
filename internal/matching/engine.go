package matching

import (
	"context"
	"ob/internal/entity"
	"ob/pkg/log"
)

type Engine struct {
	book *Book
	ch   chan *entity.OrderBookEvent
}

func NewEngine(book *Book) *Engine {
	return &Engine{
		book: book,
		ch:   make(chan *entity.OrderBookEvent),
	}
}

func (e *Engine) Run(ctx context.Context) {
	for event := range e.ch {
		log.GetLogger(ctx).Info("received event", log.Int("qty", int(event.Order.OriginalQty)), log.Int("price", int(event.Order.LimitPrice)))
		// switch event.Op {
		// case entity.OperationNewOrder:
		// 	e.book.SubmitOrder(event.Order)
		// case entity.OperationModifyOrder:
		// 	e.book.ModifyOrder(event.Order)
		// case entity.OperationCancelOrder:
		// 	e.book.CancelOrder(event.Order)
		// }
	}
}

func (e *Engine) SubmitOrder(order *entity.Order) error {
	e.ch <- &entity.OrderBookEvent{
		Order: *order,
		Op:    entity.OperationNewOrder,
	}
	return nil
}
