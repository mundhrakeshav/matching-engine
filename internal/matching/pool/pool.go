package pool

import (
	"errors"
	"ob/internal/entity"
)

var (
	errPoolExhausted    = errors.New("pool exhausted")
	errInvalidIndex     = errors.New("pool index out of bounds")
	errSlotNotAllocated = errors.New("pool slot is not allocated")
)

type Pool[T any] struct {
	values              []T
	nextFree            []entity.NodeID
	allocated           []uint64
	freeHead, nextFresh entity.NodeID
}

func New[T any](size uint64) Pool[T] {
	return Pool[T]{
		values:    make([]T, size),
		nextFree:  make([]entity.NodeID, size),
		allocated: make([]uint64, (size+63)/64),
		freeHead:  entity.NoneU64,
		nextFresh: 0,
	}
}

func (p *Pool[T]) Allocate(value T) (entity.NodeID, error) {
	if p.freeHead == entity.NoneU64 && p.nextFresh == entity.NodeID(len(p.values)) {
		return 0, errPoolExhausted
	}

	var idx entity.NodeID
	if p.freeHead != entity.NoneU64 {
		idx = p.freeHead
		p.freeHead = p.nextFree[idx]
	} else {
		idx = p.nextFresh
		p.nextFresh++
	}

	p.values[idx] = value
	p.markAllocated(idx)
	return idx, nil
}

func (p *Pool[T]) Get(idx entity.NodeID) (*T, error) {
	if idx >= entity.NodeID(len(p.values)) {
		return nil, errInvalidIndex
	}

	if !p.isAllocated(idx) {
		return nil, errSlotNotAllocated
	}

	return &p.values[idx], nil
}

/*
 */
func (p *Pool[T]) Release(idx entity.NodeID) error {
	if idx >= entity.NodeID(len(p.values)) {
		return errInvalidIndex
	}
	if !p.isAllocated(idx) {
		return errSlotNotAllocated
	}

	var zero T
	p.values[idx] = zero
	p.markFree(idx)
	p.nextFree[idx] = p.freeHead
	p.freeHead = idx
	return nil
}

func (p *Pool[T]) isAllocated(idx entity.NodeID) bool {
	return p.allocated[idx/64]&(uint64(1)<<(idx%64)) != 0
}

func (p *Pool[T]) markAllocated(idx entity.NodeID) {
	// slot = idx / 64
	// bit = idx % 64
	// allocated[slot] |= 1 << bit => allocated[slot] = allocated[slot] OR (1 << bit)
	p.allocated[idx/64] |= uint64(1) << (idx % 64)
}

func (p *Pool[T]) markFree(idx entity.NodeID) {
	// slot = idx / 64
	// bit = idx % 64
	// allocated[slot] &^= 1 << bit => allocated[slot] = allocated[slot] AND (NOT (1 << bit))
	p.allocated[idx/64] &^= uint64(1) << (idx % 64)
}
