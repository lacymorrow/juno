    // Combine results
    match (result, release_errors.is_empty()) {
        (Ok(res), true) => Ok(res),
        (Ok(_), false) => Err(AutomationError::Internal(format!("Action succeeded, but failed to release modifiers: {:?}", release_errors))),
        (Err(action_err), true) => Err(action_err),
        (Err(action_err), false) => Err(AutomationError::Internal(format!("Action failed: {}. Also failed to release modifiers: {:?}", action_err, release_errors))),
    }
