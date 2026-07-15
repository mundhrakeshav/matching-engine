package matching

import "ob/internal/entity"

type Engine struct {
	book *entity.Book
}

func NewEngine(book *entity.Book) *Engine {
	return &Engine{
		book: book,
	}
}
