#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardVisibility {
    Public,
    Private,
}

impl BoardVisibility {
    pub fn from_is_private(is_private: bool) -> Self {
        if is_private {
            Self::Private
        } else {
            Self::Public
        }
    }

    pub fn is_private(self) -> bool {
        matches!(self, Self::Private)
    }
}
