#!/usr/bin/env python3
"""QLoRA fine-tuning of Llama 3.1 8B on Racing Point operational data using Unsloth."""

import json
import sys
from pathlib import Path

DATA_DIR = Path(__file__).parent / "data"
OUTPUT_DIR = Path(__file__).parent / "output" / "lora_adapter"
TRAIN_FILE = DATA_DIR / "train.json"
EVAL_FILE = DATA_DIR / "eval.json"

# ─── Hyperparameters ────────────────────────────────────────────────────────

BASE_MODEL = "unsloth/Meta-Llama-3.1-8B-Instruct"
MAX_SEQ_LENGTH = 2048
LORA_R = 16
LORA_ALPHA = 16
LORA_DROPOUT = 0
BATCH_SIZE = 2
GRAD_ACCUM = 4
LEARNING_RATE = 2e-4
NUM_EPOCHS = 3
WARMUP_STEPS = 10
WEIGHT_DECAY = 0.01
SEED = 42

SYSTEM_PROMPT = (
    "You are James, the AI operations assistant for RacingPoint eSports and Cafe "
    "(Bandlaguda, Hyderabad). You manage 8 sim racing pods, diagnose technical issues, "
    "and help staff with billing, game launches, and hardware troubleshooting. "
    "Be concise, actionable, and specific to our setup."
)


def format_alpaca_to_chat(example: dict) -> str:
    """Convert Alpaca format to Llama 3.1 chat template."""
    instruction = example["instruction"]
    inp = example.get("input", "")
    output = example["output"]

    user_content = instruction
    if inp:
        user_content += f"\n\nAdditional context: {inp}"

    # Llama 3.1 Instruct chat format
    return (
        f"<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n"
        f"{SYSTEM_PROMPT}<|eot_id|>"
        f"<|start_header_id|>user<|end_header_id|>\n\n"
        f"{user_content}<|eot_id|>"
        f"<|start_header_id|>assistant<|end_header_id|>\n\n"
        f"{output}<|eot_id|>"
    )


def main():
    # Verify data exists
    if not TRAIN_FILE.exists():
        print(f"ERROR: Training data not found at {TRAIN_FILE}", file=sys.stderr)
        print("Run the data generation scripts first:", file=sys.stderr)
        print("  python export_training_pairs.py", file=sys.stderr)
        print("  python generate_playbook_pairs.py", file=sys.stderr)
        print("  python generate_ops_pairs.py", file=sys.stderr)
        print("  python generate_crash_pairs.py", file=sys.stderr)
        print("  python merge_dataset.py", file=sys.stderr)
        sys.exit(1)

    # Check GPU
    import torch
    if not torch.cuda.is_available():
        print("ERROR: CUDA not available. QLoRA requires a GPU.", file=sys.stderr)
        sys.exit(1)

    gpu_name = torch.cuda.get_device_name(0)
    gpu_mem = torch.cuda.get_device_properties(0).total_mem / (1024**3)
    print(f"GPU: {gpu_name} ({gpu_mem:.1f} GB)")

    # Load model with Unsloth
    from unsloth import FastLanguageModel

    print(f"\nLoading base model: {BASE_MODEL}")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=BASE_MODEL,
        max_seq_length=MAX_SEQ_LENGTH,
        dtype=None,  # auto-detect
        load_in_4bit=True,
    )

    # Add LoRA adapters
    print(f"\nAdding LoRA adapters (r={LORA_R}, alpha={LORA_ALPHA})")
    model = FastLanguageModel.get_peft_model(
        model,
        r=LORA_R,
        target_modules=[
            "q_proj", "k_proj", "v_proj", "o_proj",
            "gate_proj", "up_proj", "down_proj",
        ],
        lora_alpha=LORA_ALPHA,
        lora_dropout=LORA_DROPOUT,
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=SEED,
    )

    # Load and format dataset
    print(f"\nLoading training data from {TRAIN_FILE}")
    with open(TRAIN_FILE, "r", encoding="utf-8") as f:
        train_data = json.load(f)

    eval_data = []
    if EVAL_FILE.exists():
        with open(EVAL_FILE, "r", encoding="utf-8") as f:
            eval_data = json.load(f)

    print(f"  Train: {len(train_data)} examples")
    print(f"  Eval:  {len(eval_data)} examples")

    # Format to chat template
    from datasets import Dataset

    train_texts = [format_alpaca_to_chat(ex) for ex in train_data]
    train_dataset = Dataset.from_dict({"text": train_texts})

    eval_dataset = None
    if eval_data:
        eval_texts = [format_alpaca_to_chat(ex) for ex in eval_data]
        eval_dataset = Dataset.from_dict({"text": eval_texts})

    # Training
    from trl import SFTTrainer
    from transformers import TrainingArguments

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    print(f"\nStarting training for {NUM_EPOCHS} epochs...")
    print(f"  Effective batch size: {BATCH_SIZE * GRAD_ACCUM}")
    print(f"  Learning rate: {LEARNING_RATE}")
    print(f"  Output: {OUTPUT_DIR}")

    training_args = TrainingArguments(
        per_device_train_batch_size=BATCH_SIZE,
        gradient_accumulation_steps=GRAD_ACCUM,
        warmup_steps=WARMUP_STEPS,
        num_train_epochs=NUM_EPOCHS,
        learning_rate=LEARNING_RATE,
        fp16=not torch.cuda.is_bf16_supported(),
        bf16=torch.cuda.is_bf16_supported(),
        logging_steps=5,
        optim="adamw_8bit",
        weight_decay=WEIGHT_DECAY,
        lr_scheduler_type="cosine",
        seed=SEED,
        output_dir=str(OUTPUT_DIR),
        save_strategy="epoch",
        evaluation_strategy="epoch" if eval_dataset else "no",
        report_to="none",
    )

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=train_dataset,
        eval_dataset=eval_dataset,
        dataset_text_field="text",
        max_seq_length=MAX_SEQ_LENGTH,
        dataset_num_proc=2,
        packing=False,
        args=training_args,
    )

    trainer_stats = trainer.train()

    print(f"\nTraining complete!")
    print(f"  Total steps: {trainer_stats.global_step}")
    print(f"  Train loss: {trainer_stats.training_loss:.4f}")
    print(f"  Runtime: {trainer_stats.metrics['train_runtime']:.0f}s")

    # Save LoRA adapter
    print(f"\nSaving LoRA adapter to {OUTPUT_DIR}")
    model.save_pretrained(str(OUTPUT_DIR))
    tokenizer.save_pretrained(str(OUTPUT_DIR))

    # Quick sanity check
    print("\n--- Sanity Check ---")
    FastLanguageModel.for_inference(model)

    test_prompt = format_alpaca_to_chat({
        "instruction": "What is the IP address of pod 3?",
        "input": "",
        "output": "",
    }).rsplit("<|start_header_id|>assistant<|end_header_id|>", 1)[0] + "<|start_header_id|>assistant<|end_header_id|>\n\n"

    inputs = tokenizer(test_prompt, return_tensors="pt").to("cuda")
    outputs = model.generate(**inputs, max_new_tokens=200, temperature=0.7)
    response = tokenizer.decode(outputs[0][inputs.input_ids.shape[1]:], skip_special_tokens=True)
    print(f"Q: What is the IP address of pod 3?")
    print(f"A: {response[:300]}")

    print(f"\nAdapter saved to: {OUTPUT_DIR}")
    print("Next step: run convert_and_import.py to create GGUF and import to Ollama")


if __name__ == "__main__":
    main()
