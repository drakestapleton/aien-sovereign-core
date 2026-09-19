"""Unit tests for KV-expanded Nemotron-H architecture adapter."""

import unittest
from types import SimpleNamespace
import numpy as np

from nemotron_h_kvexp.config import kv_expansion_factor, MAX_NVIDIA_GQA_GROUP
from nemotron_h_kvexp.weight_adapters import (
    _bf16_to_f32,
    _f32_to_bf16,
    _delta,
)


class TestKvExpansion(unittest.TestCase):
    def test_expansion_factor_nemotron_30b(self):
        """Nemotron-30B: 32 query heads, 2 KV heads -> group 16 -> expansion 2."""
        cfg = SimpleNamespace(num_attention_heads=32, num_key_value_heads=2)
        factor = kv_expansion_factor(cfg)
        self.assertEqual(factor, 2)
        effective_kv = cfg.num_key_value_heads * factor
        effective_group = cfg.num_attention_heads / effective_kv
        self.assertLessEqual(effective_group, MAX_NVIDIA_GQA_GROUP)

    def test_expansion_factor_standard_gqa(self):
        """Standard GQA: 32 query heads, 4 KV heads -> group 8 -> expansion 1."""
        cfg = SimpleNamespace(num_attention_heads=32, num_key_value_heads=4)
        factor = kv_expansion_factor(cfg)
        self.assertEqual(factor, 1)

    def test_expansion_factor_large_group(self):
        """Hypothetical model: 64 query heads, 2 KV heads -> group 32 -> expansion 4."""
        cfg = SimpleNamespace(num_attention_heads=64, num_key_value_heads=2)
        factor = kv_expansion_factor(cfg)
        self.assertEqual(factor, 4)
        effective_kv = cfg.num_key_value_heads * factor
        effective_group = cfg.num_attention_heads / effective_kv
        self.assertLessEqual(effective_group, MAX_NVIDIA_GQA_GROUP)

    def test_expansion_factor_zero_kv(self):
        """Edge case: zero or negative KV heads returns 1."""
        cfg = SimpleNamespace(num_attention_heads=32, num_key_value_heads=0)
        self.assertEqual(kv_expansion_factor(cfg), 1)


class TestBf16Conversions(unittest.TestCase):
    def test_bf16_f32_roundtrip(self):
        """Verify bfloat16 to float32 bit-shift roundtrip."""
        original = np.array([0.0, 1.0, -1.0, 2.5, 0.125], dtype=np.float32)
        u16 = _f32_to_bf16(original)
        recovered = _bf16_to_f32(u16)
        np.testing.assert_allclose(recovered, original, rtol=1e-3, atol=1e-3)


class TestLoraDelta(unittest.TestCase):
    def test_delta_computation(self):
        """Verify LoRA low-rank delta calculation: scale * (B @ A)."""
        r = 4
        in_dim = 16
        out_dim = 32
        scale = 2.0

        A = np.random.randn(r, in_dim).astype(np.float32)
        B = np.random.randn(out_dim, r).astype(np.float32)

        expected = scale * (B @ A)
        actual = _delta(A, B, scale)

        self.assertEqual(actual.shape, (out_dim, in_dim))
        np.testing.assert_allclose(actual, expected, rtol=1e-6)


class TestWeightReplication(unittest.TestCase):
    def test_kv_replication_math(self):
        """Verify manual KV tensor replication semantics along head dimension."""
        n_kv = 2
        head_dim = 64
        hidden = 128
        expansion = 2

        # Create synthetic KV projection weight
        k = np.arange(n_kv * head_dim * hidden, dtype=np.float32).reshape(n_kv, head_dim, hidden)
        
        # Replicate
        k_rep = np.repeat(k, expansion, axis=0)
        self.assertEqual(k_rep.shape, (n_kv * expansion, head_dim, hidden))
        
        # Verify first and second copies match the original first head
        np.testing.assert_array_equal(k_rep[0], k[0])
        np.testing.assert_array_equal(k_rep[1], k[0])
        # Verify third and fourth copies match the original second head
        np.testing.assert_array_equal(k_rep[2], k[1])
        np.testing.assert_array_equal(k_rep[3], k[1])


if __name__ == "__main__":
    unittest.main()
