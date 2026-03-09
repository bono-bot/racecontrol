#!/usr/bin/env python3
"""Merge LoRA adapter with base model, convert to GGUF, and import into Ollama."""

import subprocess
import sys
from pathlib import Path

LORA_DIR = Path(__file__).parent / "output" / "lora_adapter"
MERGED_DIR = Path(__file__).parent / "output" / "merged_model"
GGUF_DIR = Path(__file__).parent / "output" / "gguf"
MODELFILE_PATH = Path(__file__).parent / "Modelfile"
OLLAMA_MODEL_NAME = "racing-point-ops"

BASE_MODEL = "unsloth/Meta-Llama-3.1-8B-Instruct"
QUANTIZATION = "q4_k_m"


def step_merge():
    """Step 1: Merge LoRA adapter with base model."""
    print("=" * 60)
    print("STEP 1: Merging LoRA adapter with base model")
    print("=" * 60)

    if not LORA_DIR.exists():
        print(f"ERROR: LoRA adapter not found at {LORA_DIR}", file=sys.stderr)
        print("Run train_qlora.py first.", file=sys.stderr)
        sys.exit(1)

    import torch
    from unsloth import FastLanguageModel

    print(f"Loading base model + LoRA from {LORA_DIR}")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=str(LORA_DIR),
        max_seq_length=2048,
        dtype=None,
        load_in_4bit=True,
    )

    # Merge and save as full-precision model
    MERGED_DIR.mkdir(parents=True, exist_ok=True)
    print(f"Merging and saving to {MERGED_DIR}")

    # Use Unsloth's save_pretrained_merged for efficient merge
    model.save_pretrained_merged(
        str(MERGED_DIR),
        tokenizer,
        save_method="merged_16bit",
    )
    print(f"Merged model saved to {MERGED_DIR}")


def step_convert_gguf():
    """Step 2: Convert merged model to GGUF format."""
    print("\n" + "=" * 60)
    print(f"STEP 2: Converting to GGUF ({QUANTIZATION})")
    print("=" * 60)

    if not MERGED_DIR.exists():
        print(f"ERROR: Merged model not found at {MERGED_DIR}", file=sys.stderr)
        sys.exit(1)

    GGUF_DIR.mkdir(parents=True, exist_ok=True)

    # Try using Unsloth's built-in GGUF export first
    try:
        import torch
        from unsloth import FastLanguageModel

        print(f"Loading merged model from {MERGED_DIR}")
        model, tokenizer = FastLanguageModel.from_pretrained(
            model_name=str(MERGED_DIR),
            max_seq_length=2048,
            dtype=None,
            load_in_4bit=False,
        )

        gguf_path = GGUF_DIR / f"racing-point-ops-{QUANTIZATION}.gguf"
        print(f"Exporting to {gguf_path}")

        model.save_pretrained_gguf(
            str(GGUF_DIR),
            tokenizer,
            quantization_method=QUANTIZATION,
        )
        print(f"GGUF file created: {gguf_path}")
        return gguf_path

    except Exception as e:
        print(f"Unsloth GGUF export failed: {e}")
        print("Falling back to llama.cpp conversion...")

    # Fallback: use llama.cpp convert script
    llama_cpp_dir = Path(__file__).parent / "llama.cpp"
    if not llama_cpp_dir.exists():
        print("Cloning llama.cpp for GGUF conversion...")
        subprocess.run(
            ["git", "clone", "--depth=1", "https://github.com/ggerganov/llama.cpp", str(llama_cpp_dir)],
            check=True,
        )

    # Install llama.cpp Python deps
    subprocess.run(
        [sys.executable, "-m", "pip", "install", "-r", str(llama_cpp_dir / "requirements.txt")],
        check=True,
        capture_output=True,
    )

    # Convert to GGUF (F16 first)
    f16_gguf = GGUF_DIR / "racing-point-ops-f16.gguf"
    print(f"Converting to F16 GGUF: {f16_gguf}")
    subprocess.run(
        [
            sys.executable,
            str(llama_cpp_dir / "convert_hf_to_gguf.py"),
            str(MERGED_DIR),
            "--outfile", str(f16_gguf),
            "--outtype", "f16",
        ],
        check=True,
    )

    # Quantize to Q4_K_M
    quantized_gguf = GGUF_DIR / f"racing-point-ops-{QUANTIZATION}.gguf"
    print(f"Quantizing to {QUANTIZATION}: {quantized_gguf}")

    # Try to find llama-quantize binary
    quantize_bin = llama_cpp_dir / "build" / "bin" / "llama-quantize"
    if not quantize_bin.exists():
        quantize_bin = llama_cpp_dir / "llama-quantize"
    if not quantize_bin.exists():
        # Build llama.cpp
        print("Building llama.cpp quantize tool...")
        build_dir = llama_cpp_dir / "build"
        build_dir.mkdir(exist_ok=True)
        subprocess.run(["cmake", "..", "-DCMAKE_BUILD_TYPE=Release"], cwd=str(build_dir), check=True)
        subprocess.run(["cmake", "--build", ".", "--config", "Release", "-j"], cwd=str(build_dir), check=True)
        quantize_bin = build_dir / "bin" / "llama-quantize"

    subprocess.run(
        [str(quantize_bin), str(f16_gguf), str(quantized_gguf), QUANTIZATION.upper()],
        check=True,
    )

    # Clean up F16 file
    f16_gguf.unlink(missing_ok=True)
    print(f"GGUF file created: {quantized_gguf}")
    return quantized_gguf


def step_import_ollama():
    """Step 3: Import GGUF model into Ollama."""
    print("\n" + "=" * 60)
    print("STEP 3: Importing into Ollama")
    print("=" * 60)

    # Find the GGUF file
    gguf_files = list(GGUF_DIR.glob(f"*{QUANTIZATION}*.gguf"))
    if not gguf_files:
        # Check for unsloth output pattern
        gguf_files = list(GGUF_DIR.glob("*.gguf"))
    if not gguf_files:
        print(f"ERROR: No GGUF file found in {GGUF_DIR}", file=sys.stderr)
        sys.exit(1)

    gguf_path = gguf_files[0]
    print(f"GGUF file: {gguf_path} ({gguf_path.stat().st_size / (1024**3):.2f} GB)")

    # Verify Modelfile exists
    if not MODELFILE_PATH.exists():
        print(f"ERROR: Modelfile not found at {MODELFILE_PATH}", file=sys.stderr)
        sys.exit(1)

    # Read and update Modelfile with correct GGUF path
    modelfile_content = MODELFILE_PATH.read_text(encoding="utf-8")
    # Replace placeholder with actual path
    updated = modelfile_content.replace(
        "FROM ./output/gguf/racing-point-ops-q4_k_m.gguf",
        f"FROM {gguf_path}",
    )

    # Write temporary Modelfile with absolute path
    temp_modelfile = GGUF_DIR / "Modelfile"
    temp_modelfile.write_text(updated, encoding="utf-8")

    # Create Ollama model
    print(f"Creating Ollama model: {OLLAMA_MODEL_NAME}")
    result = subprocess.run(
        ["ollama", "create", OLLAMA_MODEL_NAME, "-f", str(temp_modelfile)],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"ERROR: ollama create failed:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)

    print(f"Model created: {OLLAMA_MODEL_NAME}")
    print(result.stdout)

    # Verify
    print(f"\nVerifying model...")
    result = subprocess.run(
        ["ollama", "list"],
        capture_output=True,
        text=True,
    )
    if OLLAMA_MODEL_NAME in result.stdout:
        print(f"SUCCESS: {OLLAMA_MODEL_NAME} is available in Ollama!")
    else:
        print(f"WARNING: {OLLAMA_MODEL_NAME} not found in ollama list")


def step_test():
    """Step 4: Test the model with domain queries."""
    print("\n" + "=" * 60)
    print("STEP 4: Testing model")
    print("=" * 60)

    test_queries = [
        "What is the IP address of pod 5?",
        "How does billing work at Racing Point?",
        "Assetto Corsa crashed on pod 3 with exit code -1073741819. What should I do?",
        "What wheelbase do we use?",
        "How do I deploy rc-agent to the pods?",
    ]

    for query in test_queries:
        print(f"\nQ: {query}")
        result = subprocess.run(
            ["ollama", "run", OLLAMA_MODEL_NAME, query],
            capture_output=True,
            text=True,
            timeout=60,
        )
        if result.returncode == 0:
            # Truncate long responses
            response = result.stdout.strip()
            if len(response) > 300:
                response = response[:300] + "..."
            print(f"A: {response}")
        else:
            print(f"ERROR: {result.stderr}")


def main():
    print(f"Racing Point Ops LLM — GGUF Conversion & Ollama Import")
    print(f"Base: {BASE_MODEL}")
    print(f"Quantization: {QUANTIZATION}")
    print(f"Target: ollama/{OLLAMA_MODEL_NAME}\n")

    steps = {
        "merge": step_merge,
        "gguf": step_convert_gguf,
        "import": step_import_ollama,
        "test": step_test,
    }

    # Allow running individual steps
    if len(sys.argv) > 1:
        step_name = sys.argv[1]
        if step_name in steps:
            steps[step_name]()
            return
        else:
            print(f"Unknown step: {step_name}. Available: {', '.join(steps.keys())}")
            sys.exit(1)

    # Run all steps
    step_merge()
    step_convert_gguf()
    step_import_ollama()
    step_test()

    print("\n" + "=" * 60)
    print("ALL DONE! racing-point-ops model is ready.")
    print("=" * 60)


if __name__ == "__main__":
    main()
