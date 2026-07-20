package pool

import (
	"errors"
	"fmt"
	"ob/internal/entity"

	"testing"
)

func restingOrder(id uint64) entity.RestingOrder {
	return entity.RestingOrder{ID: entity.OrderID(id)}
}

func TestAllocate_FreshAllocationsAreSequential(t *testing.T) {
	for _, size := range []uint64{1, 2, 5, 100} {
		t.Run(fmt.Sprintf("size_%d", size), func(t *testing.T) {
			pool := New[entity.RestingOrder](size)

			for i := uint64(0); i < size; i++ {
				idx, err := pool.Allocate(restingOrder(i))
				if err != nil {
					t.Fatalf("allocate %d: %v", i, err)
				}
				if idx != entity.NodeID(i) {
					t.Fatalf("index = %d, want %d", idx, i)
				}
				if pool.nextFresh != entity.NodeID(i+1) {
					t.Fatalf("nextFresh = %d, want %d", pool.nextFresh, i+1)
				}
				if pool.freeHead != entity.NodeID(entity.NoneU64) {
					t.Fatalf("freeHead = %d, want  entity.None", pool.freeHead)
				}
			}
		})
	}
}

func TestRelease_ReusesSlotAndClearsValue(t *testing.T) {
	pool := New[string](1)
	idx, err := pool.Allocate("first")
	if err != nil {
		t.Fatalf("allocate: %v", err)
	}

	if err := pool.Release(idx); err != nil {
		t.Fatalf("release: %v", err)
	}
	if _, err := pool.Get(idx); err == nil {
		t.Fatal("get returned value for released slot, want error")
	}

	reused, err := pool.Allocate("second")
	if err != nil {
		t.Fatalf("reallocate: %v", err)
	}
	if reused != idx {
		t.Fatalf("reused index = %d, want %d", reused, idx)
	}
	value, err := pool.Get(reused)
	if err != nil {
		t.Fatalf("get reused slot: %v", err)
	}
	if *value != "second" {
		t.Fatalf("reused value = %q, want \"second\"", *value)
	}
}

func TestRelease_RejectsUnallocatedAndDoubleReleasedSlots(t *testing.T) {
	pool := New[int](65)

	if err := pool.Release(64); !errors.Is(err, errSlotNotAllocated) {
		t.Fatalf("release unallocated slot: got %v, want %v", err, errSlotNotAllocated)
	}

	idx, err := pool.Allocate(1)
	if err != nil {
		t.Fatalf("allocate: %v", err)
	}
	if err := pool.Release(idx); err != nil {
		t.Fatalf("release: %v", err)
	}
	if err := pool.Release(idx); !errors.Is(err, errSlotNotAllocated) {
		t.Fatalf("double release: got %v, want %v", err, errSlotNotAllocated)
	}

	if err := pool.Release(65); !errors.Is(err, errInvalidIndex) {
		t.Fatalf("release out-of-bounds slot: got %v, want %v", err, errInvalidIndex)
	}
}
