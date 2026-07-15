package matching

import "ob/internal/entity"

type Engine struct {
	book *entity.Book
	ch   chan *entity.OrderBookEvent
}

func NewEngine(book *entity.Book) *Engine {
	return &Engine{
		book: book,
		ch:   make(chan *entity.OrderBookEvent),
	}
}

func (e *Engine) Run() {}

func (e *Engine) AddToChannel(event *entity.OrderBookEvent) {
	e.ch <- event
}
