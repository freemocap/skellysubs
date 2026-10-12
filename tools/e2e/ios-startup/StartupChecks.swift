import XCTest

final class StartupChecks: XCTestCase {
    func testColdStartsAndResumeWithoutReload() {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.freemocap.skellyspeak")
        for run in 1...5 {
            app.terminate()
            app.launch()
            checkReady(app, stage: "cold \(run)")
        }
        for run in 1...3 {
            XCUIDevice.shared.press(.home)
            app.activate()
            checkReady(app, stage: "resume \(run)")
        }
    }

    private func checkReady(_ app: XCUIApplication, stage: String) {
        let failure = app.descendants(matching: .any).matching(NSPredicate(format:
            "label IN %@ OR label BEGINSWITH %@ OR label BEGINSWITH %@",
            ["Retry startup", "Reload app", "Reload view", "Configuration could not be loaded"],
            "Could not open SkellySpeak:", "This view crashed:")).firstMatch
        let welcome = app.staticTexts["Welcome to SkellySpeak"].firstMatch
        let readyOrFailed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            welcome.exists || failure.exists
        }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [readyOrFailed], timeout: 30), .completed, stage)
        XCTAssertFalse(failure.exists, "Error screen at \(stage); no retry/reload attempted")
        XCTAssertTrue(welcome.exists, "Onboarding absent at \(stage)")
        // Verify the first render stays healthy, not just that a label flashed.
        Thread.sleep(forTimeInterval: 2)
        XCTAssertFalse(failure.exists, "Error after first render at \(stage)")
        XCTAssertTrue(welcome.exists, "Onboarding disappeared at \(stage)")
        print("PASS \(stage)")
    }
}
