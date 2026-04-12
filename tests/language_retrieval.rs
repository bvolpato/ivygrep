use ivygrep::embedding::create_hash_model;
use ivygrep::indexer::index_workspace;
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;
use serial_test::serial;

#[test]
#[serial]
fn parser_backed_languages_are_retrievable() {
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join("InvoiceService.java"),
        "public class InvoiceService {\n    public int calculateTotal(int subtotal) {\n        return subtotal * 2;\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("BillingService.cs"),
        "public class BillingService {\n    public decimal CalculateTotal(decimal subtotal) {\n        return subtotal * 1.2m;\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("InvoiceService.php"),
        "<?php\nclass InvoiceService {\n    public function calculateTotal(float $subtotal): float {\n        return $subtotal * 1.2;\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("invoice_service.rb"),
        "module Billing\n  class InvoiceService\n    def calculate_total(subtotal)\n      subtotal * 1.2\n    end\n  end\nend\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("InvoiceService.swift"),
        "struct InvoiceService {\n    func calculateTotal(subtotal: Double) -> Double {\n        subtotal * 1.2\n    }\n}\n",
    )
    .unwrap();

    let workspace = Workspace::resolve(repo.path()).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();

    for (query, expected_file) in [
        ("where is calculate total in java", "InvoiceService.java"),
        ("where is calculate total in csharp", "BillingService.cs"),
        ("where is calculate total in php", "InvoiceService.php"),
        ("where is calculate total in ruby", "invoice_service.rb"),
        ("where is calculate total in swift", "InvoiceService.swift"),
    ] {
        let hits = hybrid_search(
            &workspace,
            query,
            Some(model.as_ref()),
            &SearchOptions {
                limit: Some(5),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(
            hits.iter()
                .any(|hit| hit.file_path.to_string_lossy() == expected_file),
            "query {query:?} should return {expected_file}, got {hits:#?}"
        );
    }
}
