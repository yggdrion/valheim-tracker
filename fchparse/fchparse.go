// Package fchparse reads the achievement-relevant part of a Valheim 1.0 .fch
// (profile v46+). Layout follows Melichor/ValheimSaveParser achi_hunter.py parse().
package fchparse

import (
	"encoding/binary"
	"errors"
	"fmt"
	"math"
)

const (
	MinVersion = 46
	StatCount  = 205
	SetCount   = 10
)

type Set struct {
	Stats     []float32          `json:"stats"`
	Worlds    map[string]float32 `json:"worlds"`
	Kills     map[string]float32 `json:"kills"` // enemy dict 0 = MixedAndTotal
	PickedUp  map[string]float32 `json:"picked_up"`
	Crafted   map[string]float32 `json:"crafted"`
	Pickables map[string]float32 `json:"pickables"`
	FoodsEat  map[string]float32 `json:"foods_eaten"`
	Built     map[string]float32 `json:"pieces_built"`
}

type Profile struct {
	Version int   `json:"version"`
	Sets    []Set `json:"sets"`
}

type reader struct {
	d []byte
	p int
}

var errEOF = errors.New("fch: unexpected end of data")

func (r *reader) need(n int) {
	if n < 0 || r.p+n > len(r.d) {
		panic(errEOF)
	}
}
func (r *reader) i32() int {
	r.need(4)
	v := int32(binary.LittleEndian.Uint32(r.d[r.p:]))
	r.p += 4
	return int(v)
}
func (r *reader) f32() float32 {
	r.need(4)
	v := math.Float32frombits(binary.LittleEndian.Uint32(r.d[r.p:]))
	r.p += 4
	return v
}
func (r *reader) str() string { // .NET 7-bit length prefix
	n, shift := 0, 0
	for {
		r.need(1)
		b := r.d[r.p]
		r.p++
		n |= int(b&0x7f) << shift
		shift += 7
		if b < 0x80 {
			break
		}
	}
	r.need(n)
	s := string(r.d[r.p : r.p+n])
	r.p += n
	return s
}
func (r *reader) sdict() map[string]float32 {
	n := r.i32()
	if n < 0 || n > len(r.d) {
		panic(errEOF)
	}
	m := make(map[string]float32, n)
	for i := 0; i < n; i++ {
		k := r.str()
		m[k] = r.f32()
	}
	return m
}

// Parse decodes raw .fch bytes. Never writes anything.
func Parse(raw []byte) (p *Profile, err error) {
	defer func() {
		if x := recover(); x != nil {
			p, err = nil, fmt.Errorf("fch: parse failed: %v", x)
		}
	}()
	if len(raw) < 4 {
		return nil, errEOF
	}
	n := int(int32(binary.LittleEndian.Uint32(raw)))
	if n < 0 || 4+n > len(raw) {
		return nil, errors.New("fch: length header does not match file size")
	}
	r := &reader{d: raw[4 : 4+n]}
	ver := r.i32()
	if ver < MinVersion {
		return nil, fmt.Errorf("fch: profile version %d older than supported %d", ver, MinVersion)
	}
	nStats, nSets := r.i32(), r.i32()
	if nStats != StatCount || nSets != SetCount {
		return nil, fmt.Errorf("fch: %d stats in %d sets, expected %d in %d: layout changed", nStats, nSets, StatCount, SetCount)
	}
	p = &Profile{Version: ver}
	for i := 0; i < nSets; i++ {
		var s Set
		s.Stats = make([]float32, nStats)
		for j := range s.Stats {
			s.Stats[j] = r.f32()
		}
		s.Worlds = r.sdict()
		r.sdict() // known world keys
		r.sdict() // known commands
		enemy := make([]map[string]float32, r.i32())
		for j := range enemy {
			enemy[j] = r.sdict()
		}
		if len(enemy) == 0 {
			return nil, errors.New("fch: no enemy stat dict")
		}
		s.Kills = enemy[0]
		s.PickedUp = r.sdict()
		s.Crafted = r.sdict()
		s.Pickables = r.sdict()
		s.FoodsEat = r.sdict()
		s.Built = r.sdict()
		p.Sets = append(p.Sets, s)
	}
	return p, nil
}
