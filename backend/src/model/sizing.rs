use bigdecimal::BigDecimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizingConfig {
    pub bankroll: BigDecimal,
    pub fraction: BigDecimal,
    pub max_size: BigDecimal,
    pub min_size: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizingEngine {
    config: SizingConfig,
}

impl SizingEngine {
    pub fn new(config: SizingConfig) -> Self {
        Self { config }
    }

    pub fn default_for_bankroll(bankroll: BigDecimal) -> Self {
        Self::new(SizingConfig {
            bankroll,
            fraction: BigDecimal::from(1) / BigDecimal::from(2),
            max_size: BigDecimal::from(1),
            min_size: BigDecimal::from(1) / BigDecimal::from(2),
        })
    }

    pub fn suggest_size(
        &self,
        win_probability: &BigDecimal,
        contract_price: &BigDecimal,
    ) -> Option<BigDecimal> {
        let zero = BigDecimal::from(0);
        let one = BigDecimal::from(1);

        if win_probability <= &zero || win_probability >= &one {
            return None;
        }
        if contract_price <= &zero || contract_price >= &one {
            return None;
        }

        let edge = win_probability - contract_price;
        if edge <= zero {
            return None;
        }

        let raw_fraction = edge / (one - contract_price);
        let size = self.config.bankroll.clone() * raw_fraction * self.config.fraction.clone();
        if size < self.config.min_size {
            return None;
        }
        if size > self.config.max_size {
            return Some(self.config.max_size.clone());
        }

        Some(size)
    }
}
