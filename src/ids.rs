use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(i64);

        impl $name {
            pub fn new(value: i64) -> Option<Self> {
                (value > 0).then_some(Self(value))
            }

            pub fn get(self) -> i64 {
                self.0
            }
        }

        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_type!(ArticleId);
id_type!(LingqLessonId);
id_type!(CollectionId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_ids_reject_non_positive_values() {
        assert!(ArticleId::new(0).is_none());
        assert!(ArticleId::new(-1).is_none());
        assert_eq!(ArticleId::new(42).unwrap().get(), 42);
    }
}
