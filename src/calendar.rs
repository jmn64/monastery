use bevy::prelude::*;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Month {
    #[default]
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl Month {
    pub fn days_in_month(&self, year: i32) -> i32 {
        match self {
            Month::January => 31,
            Month::February => if year % 4 == 0 { 29 } else { 28 },
            Month::March => 31,
            Month::April => 30,
            Month::May => 31,
            Month::June => 30,
            Month::July => 31,
            Month::August => 31,
            Month::September => 30,
            Month::October => 31,
            Month::November => 30,
            Month::December => 31,
        }
    }

    pub fn next(&self) -> Month {
        match self {
            Month::January => Month::February,
            Month::February => Month::March,
            Month::March => Month::April,
            Month::April => Month::May,
            Month::May => Month::June,
            Month::June => Month::July,
            Month::July => Month::August,
            Month::August => Month::September,
            Month::September => Month::October,
            Month::October => Month::November,
            Month::November => Month::December,
            Month::December => Month::January,
        }
    }
}

pub struct Date {
    pub year: i32,
    pub month: Month,
    pub day: i32,
}

impl Date {
    pub fn next_day(&mut self) {
        let max_days = self.month.days_in_month(self.year);
        // Go to January 1st and increment year if last day of December
        if self.month == Month::December && self.day == max_days {
            self.year += 1;
            self.month = self.month.next();
            self.day = 1;
        } else if self.day == max_days {
            self.month = self.month.next();
            self.day = 1;
        } else {
            self.day += 1;
        }
    }

    pub fn to_absolute_days(&self) -> i64 {
        let prior_year = (self.year - 1) as i64;
        let mut days = prior_year * 365 + (prior_year / 4);

        let mut m = Month::January;
        while m != self.month {
            days += m.days_in_month(self.year) as i64;
            m = m.next();
        }

        days += (self.day - 1) as i64;

        days
    }

    pub fn days_between(&self, other: &Date) -> i64 {
        (self.to_absolute_days() - other.to_absolute_days()).abs()
    }
}

#[derive(Component)]
pub struct Age {
    pub start_date: Date
}